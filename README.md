# Lychnos

Open-source, local-first ambient AI companion platform.

## Canonical visual identity

The canonical Lychnos body reference is stored at:

`assets/canon/lychnos-body-v1.webp`

This reference defines the baseline Lychnos visual identity across desktop/phone presence and the future portable body: glossy segmented black shell, cyan/blue expressive face, floating desktop presence, and the compact portable form shown in the reference sheet.

Future outfits, armour, materials, glow states, and theme-specific appearances should be built on top of this canonical identity rather than replacing it.


## Project documentation

- [Vision](VISION.md)
- [Architecture](ARCHITECTURE.md)
- [Roadmap](ROADMAP.md)
- [Security model](SECURITY.md)
- [Current state](CURRENT-STATE.md)
- [Architecture decisions](docs/decisions/)

The codebase is still foundation-first. Typed interaction, local push-to-talk capture, and local speech-to-text now exist, while real Omarchy monitoring, a real AI provider, text-to-speech, wake-word activation, and system execution remain behind explicit adapter/security boundaries.

## Omarchy development install

On an Omarchy machine, install the current desktop shell with:

```bash
./scripts/install-omarchy.sh
```

The installer builds the user-level Lychnos runtime and shell, installs their assets and Omarchy bar integration into user-owned locations, adds a desktop launcher, and installs the `lychnos` command.

For local voice input and spoken replies, install the user-level STT/TTS stacks once with:

```bash
./scripts/install-local-stt.sh
./scripts/install-local-tts.sh
```

These install multilingual whisper.cpp speech recognition and Piper text-to-speech plus the current Lychnos baseline voice under Lychnos-owned user directories; neither requires `sudo`.

After installation:

```bash
lychnos
lychnos status
lychnos stop
lychnos restart
lychnos logs
```

Running `lychnos` with no arguments starts the Lychnos runtime and shell, or restores the existing shell when both are already running.

## License

Lychnos is dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless explicitly stated otherwise, any contribution intentionally submitted for inclusion in Lychnos is licensed under the same terms.
