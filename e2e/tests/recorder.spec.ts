import { expect, Page, test } from '@playwright/test';

// One serial journey through the recorder: the tests build on each other's
// state (takes accumulate in the throwaway episode) exactly like a session.
test.describe.configure({ mode: 'serial' });

async function recordTake(page: Page, ms = 1500) {
  await page.keyboard.press('Space');
  await expect(page.locator('body')).toHaveClass('countdown');
  // 3-2-1 at 700ms per tick
  await expect(page.locator('body')).toHaveClass('recording', { timeout: 10_000 });
  await page.waitForTimeout(ms);
  await page.keyboard.press('Space');
  await expect(page.locator('body')).toHaveClass('review');
}

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.clip-row')).toHaveCount(5);
});

test('boots: clips, teleprompter, takes rail, mic picker', async ({ page }) => {
  await expect(page.locator('body')).toHaveClass('idle');
  await expect(page.locator('#promptText')).toContainText('Rust versus Python');
  await expect(page.locator('.clip-row.active .name')).toHaveText('title / hook');
  // fresh episode: nothing archived yet
  await expect(page.locator('.rail-empty')).toBeVisible();
  await expect(page.locator('#reviewCard')).toBeHidden();
  // fake device shows up in the input picker once permission is granted
  await expect(page.locator('#micSelect option')).not.toHaveCount(0);
});

test('recording protects the script; review shows AI feedback beside it', async ({ page }) => {
  // idle: teleprompter at full size
  const fontSize = () => page.locator('#promptText').evaluate((el) => getComputedStyle(el).fontSize);
  expect(await fontSize()).toBe('42px');

  await recordTake(page);

  // review: card + coach visible, script SHRUNK BUT NEVER HIDDEN
  // (poll: the font-size animates over 0.2s)
  await expect(page.locator('#reviewCard')).toBeVisible();
  await expect(page.locator('#promptText')).toBeVisible();
  await expect.poll(fontSize).toBe('21px');
  await expect(page.locator('#coachHdr')).toContainText('COACH · take 001', { timeout: 20_000 });
  await expect(page.locator('#coachHdr')).toContainText('peak');
  await expect(page.locator('#coachNotes li').first()).toBeVisible();
  // the take is already archived — it appears in the rail before any keep
  await expect(page.locator('.take-row .file').first()).toHaveText('take_001');
  await expect(page.locator('.take-row .meta').first()).toContainText('peak');
});

test('enter keeps the take: approved badge + clip gains audio', async ({ page }) => {
  await recordTake(page);
  await expect(page.locator('#coachHdr')).toContainText('take 002', { timeout: 20_000 });
  await page.keyboard.press('Enter');
  await expect(page.locator('body')).toHaveClass('idle');
  await expect(page.locator('#status')).toContainText('saved');
  // newest first: take_002 on top, wearing the approved badge
  const first = page.locator('.take-row').first();
  await expect(first.locator('.file')).toHaveText('take_002');
  await expect(first.locator('.badge')).toHaveText('✓');
  await expect(page.locator('.clip-row.active .dot.has-take')).toBeVisible();
});

test('takes stay newest-first and the API agrees', async ({ page, request }) => {
  await expect(page.locator('.take-row .file').first()).toHaveText('take_002');
  const files = (await (await request.get('/api/takes/title__hook')).json()).map(
    (t: { file: string }) => t.file,
  );
  const sorted = [...files].sort().reverse();
  expect(files).toEqual(sorted);
  expect(files[0]).toBe('take_002.mp3');
});

test('clicking a take discloses its saved AI review inline', async ({ page }) => {
  const row = page.locator('.take-row').first();
  await expect(row.locator('.take-detail .detail-inner')).not.toBeInViewport();
  await row.click();
  await expect(row).toHaveClass(/open/);
  await expect(row.locator('.coaching li').first()).toBeVisible();
  // one open at a time: opening another closes the first
  const second = page.locator('.take-row').nth(1);
  await second.click();
  await expect(second).toHaveClass(/open/);
  await expect(row).not.toHaveClass(/open/);
});

test('approve an older take from the rail moves the badge', async ({ page }) => {
  const older = page.locator('.take-row', { hasText: 'take_001' });
  await older.locator('button.approve').click();
  await expect(page.locator('#status')).toContainText('approved take_001.mp3');
  await expect(older.locator('.badge')).toHaveText('✓');
  await expect(page.locator('.take-row', { hasText: 'take_002' }).locator('button.approve')).toBeVisible();
});

test('audition toggles playback from the rail', async ({ page }) => {
  const play = page.locator('.take-row').first().locator('button.play');
  await expect(play).toHaveText('▶');
  await play.click();
  await expect(play).toHaveText('⏸');
  await play.click();
  await expect(play).toHaveText('▶');
});

test('retake (r) goes straight back to the countdown', async ({ page }) => {
  await recordTake(page);
  await page.keyboard.press('r');
  await expect(page.locator('body')).toHaveClass('countdown');
  await expect(page.locator('body')).toHaveClass('recording', { timeout: 10_000 });
  await page.keyboard.press('Space');
  await expect(page.locator('body')).toHaveClass('review');
});

test('arrow keys switch clips and the teleprompter follows', async ({ page }) => {
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('.clip-row.active .name')).toHaveText('good / explain');
  await expect(page.locator('#promptText')).not.toContainText('Rust versus Python');
  await page.keyboard.press('ArrowUp');
  await expect(page.locator('.clip-row.active .name')).toHaveText('title / hook');
});
