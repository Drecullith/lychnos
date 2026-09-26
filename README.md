# Lychnos

Open-source, local-first ambient AI companion platform.

## Canonical visual identity

The canonical Lychnos body reference is stored at:

`assets/canon/lychnos-body-v1.webp`

This reference defines the baseline Lychnos visual identity across desktop/phone presence and the future portable body: glossy segmented black shell, cyan/blue expressive face, floating desktop presence, and the compact portable form shown in the reference sheet.

Future outfits, armour, materials, glow states, and theme-specific appearances should be built on top of this canonical identity rather than replacing it.


## Project documentation

For a new development chat/session, start with:

- [Start here](START-HERE.md)
- [Project canon](PROJECT-CANON.md)
- [Development handoff](DEVELOPMENT-HANDOFF.md)
- [Continuity protocol](docs/CONTINUITY-PROTOCOL.md)

Then use the broader project documentation:

- [Vision](VISION.md)
- [Architecture](ARCHITECTURE.md)
- [Roadmap](ROADMAP.md)
- [Security model](SECURITY.md)
- [Current state](CURRENT-STATE.md)
- [Architecture decisions](docs/decisions/)

The codebase is still foundation-first. Typed interaction, local push-to-talk capture, local speech-to-text, local speech output, and the first real local conversational brain now exist. Lychnos uses capability-aware intelligence routing so desktops, phones, and future Pocket bodies can choose different local-brain implementations without changing identity, memory, persona, permissions, or initiative. Omarchy/Linux currently has the first working llama.cpp adapter; additional OS/mobile adapters, real contextual collectors, account-backed intelligence bridges, wake-word activation, and system execution remain behind explicit adapter/security boundaries.

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

For the first desktop LocalBrain adapter, install llama.cpp and a capability-appropriate local model with:

```bash
./scripts/install-local-brain-llama.sh
```

The desktop installer chooses Tiny / Compact / Standard / Large from available memory, or accepts an explicit class such as `--class compact`. The selected runtime/model is written only to the machine-local `~/.config/lychnos/brain.env`; it is not a universal Lychnos requirement. Android, iOS, Windows, macOS, and Pocket bodies may use different adapters behind the same core contracts.

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
