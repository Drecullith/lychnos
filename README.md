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

The current codebase is still in the machine-independent foundation phase. Real Omarchy monitoring, AI-provider integration, voice, and system execution are intentionally deferred until the core safety boundaries are stable.

## Omarchy development install

On an Omarchy machine, install the current desktop shell with:

```bash
./scripts/install-omarchy.sh
```

The installer builds the shell, installs its assets and Omarchy bar integration into user-owned locations, adds a desktop launcher, and installs the `lychnos` command.

After installation:

```bash
lychnos
lychnos status
lychnos stop
lychnos restart
lychnos logs
```

Running `lychnos` with no arguments launches Lychnos or restores the existing shell.

## License

Lychnos is dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless explicitly stated otherwise, any contribution intentionally submitted for inclusion in Lychnos is licensed under the same terms.
