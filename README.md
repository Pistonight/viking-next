# nx-decomp-tools-viking

> This is the rewrite and spiritual successor of [nx-decomp-tools](https://github.com/open-ead/nx-decomp-tools).
> Everything will be merged into that repo when ready.

Tools for various NX decompilation (more precisely, matching reimplementation) projects.

The tools were rewritten in November 2026. The goals of the rewrite are:
- **Deduplication**: Combine the Python and Rust tools into a single Python package
  with a Rust extension module. Functionality is either implemented in Python,
  or implemented in Rust and exposed as a Python API.
- **Unification**: While the old repo was inactive, the SMO decomp community forked it,
  adding more bytecode checks and changing the file list format. This rewrite is a unified version
  that ports these improvements back while staying backward compatible.
- **Distribution**: The old tool required downstream projects to install it in a certain
  way and hard-coded many paths. The rewrite ships as a single Python package
  that can be installed with any Python package manager. The TOML config file makes it
  work with any project structure and has defaults that are reasonable and backward compatible.

## Installation and CLI usage
See [Setup](./docs/SETUP.md).

## Contributing
See [Contributing](./docs/CONTRIBUTING.md).

## Credits
[leoetlino](https://github.com/leoetlino) and all contributors of nx-decomp-tools.

Projects that use these tools:
- [*Breath of the Wild*](https://github.com/zeldaret/botw)
- [*Super Mario Odyssey*](https://github.com/MonsterDruide1/OdysseyDecomp)
  - (note: Odyssey uses a fork of the old tools from before the rewrite)
