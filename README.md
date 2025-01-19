# WASM + Rust + Vite Demo

This project demonstrates:
- How to bundle WebAssembly (WASM) modules written in Rust into a web application using Vite bundler
- How to organize a monorepo using pnpm workspaces to manage WASM, library, and application packages

## Prerequisites

Before you begin, ensure you have the following installed:
- [Rust](https://www.rust-lang.org/tools/install)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org/)
- [pnpm](https://pnpm.io/installation)

## Project Structure
```
.
├── packages/
│   ├── app/          # Frontend application
│   └── library/      # TypeScript library that uses and reexports the wasm
│       └── wasm/     # Generated WASM output
├── src/              # Rust source code
│   └── lib.rs        # Rust WASM module source
├── package.json      # Workspace configuration
└── pnpm-workspace.yaml  # PNPM workspace configuration
```
The project uses pnpm workspaces to organize the codebase into three main parts:
1. Rust WASM module (root/src)
2. TypeScript wrapper library (packages/library)
3. Application (packages/app)

This workspace structure allows for:
- Independent versioning of packages
- Simplified dependency management
- Local package linking for development
- Centralized build scripts

## Getting Started

1. Clone the repository:
```bash
git clone <repository-url>
cd <project-directory>
```

2. Install dependencies:
```bash
pnpm install
```

3. Build the project:
```bash
pnpm run build
```

This command executes the following steps:
- `build:wasm`: Compiles Rust code to WebAssembly
- `build:library`: Builds the TypeScript wrapper library
- `build:app`: Builds the application

4. Run the script with node:
```bash
pnpm run start
```

## Development Workflow

### Rust WASM Module
The Rust code in `src/lib.rs` contains the WebAssembly module implementation. When you run `build:wasm`, `wasm-pack` compiles this code and generates:
- WebAssembly binary (`.wasm`)
- JavaScript bindings
- TypeScript type definitions

These files are output to `packages/library/wasm/`.

### TypeScript Library
Located in `packages/library`, this wrapper provides:
- Type-safe interface to the WASM module
- Additional utility functions
- Easy integration with JavaScript/TypeScript applications

### Application
The frontend application in `packages/app` demonstrates:
- How to import and initialize the WASM module
- Integration with the TypeScript library
- Usage of WASM functions in a web application

## Project Commands

- `pnpm run build`: Build all components
- `pnpm run build:wasm`: Build only the WASM module
- `pnpm run build:library`: Build only the TypeScript library
- `pnpm run build:app`: Build only the frontend application
- `pnpm run start`: Start the development server

## License

ISC