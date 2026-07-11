# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/security-union/videoeditor/compare/v0.1.1...v0.2.0) - 2026-07-11

### Added

- [SFX:] timeline lines — mix sound effects at absolute offsets ([#11](https://github.com/security-union/videoeditor/pull/11))
- *(voice)* local speech stack by default — whisper.cpp STT + piper TTS ([#10](https://github.com/security-union/videoeditor/pull/10))
- videoeditor-genai — typed image-gen clients (grok + imagen) and an `image` subcommand ([#8](https://github.com/security-union/videoeditor/pull/8))

### Other

- web-based narration recorder (`videoeditor record`) ([#9](https://github.com/security-union/videoeditor/pull/9))

## [0.1.1](https://github.com/security-union/videoeditor/compare/v0.1.0...v0.1.1) - 2026-07-07

### Fixed

- *(cli)* --version carries build provenance (git hash, dirty flag) ([#4](https://github.com/security-union/videoeditor/pull/4))
