# WASM + Rust + Vite Raycaster

This project is a raycaster written in Rust WASM.

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
│   └── wasm/     # Generated WASM output
├── src/              # Rust source code
│   └── lib.rs        # Rust WASM module source
├── package.json      # Workspace configuration
└── pnpm-workspace.yaml  # PNPM workspace configuration
```
The project uses pnpm workspaces to organize the codebase into two main parts:
1. Rust WASM module (root/src)
3. Application (packages/app)

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

These files are output to `packages/app/wasm/`.

### Application
The frontend application in `packages/app` demonstrates:
- How to import and initialize the WASM module
- Integration with the TypeScript library
- Usage of WASM functions in a web application

## Project Commands

- `pnpm run build`: Build all components
- `pnpm run build:wasm`: Build only the WASM module
- `pnpm run build:app`: Build only the frontend application
- `pnpm run start`: Start the development server

## Running in development

- `pnpm run dev`: Run development server, observe output at `http://localhost:5173`
