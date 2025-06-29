# Source Code Directory

This directory contains the source code for the Amazon Rose Forest project.

## Overview

The `src` directory houses the core logic and functionalities of the Amazon Rose Forest system. It is organized into several modules, each responsible for a specific aspect of the platform, such as federated learning, data storage, and AI capabilities. The code is primarily written in Rust.

Refer to `docs/architecture/overview.md` for a higher-level architectural overview of the entire system.

## Directory Structure

The main components within the `src` directory are:

- `core/`: Contains the fundamental building blocks of the system.
  - `config/`: Configuration management.
  - `dht/`: Distributed Hash Table implementation for decentralized data management.
  - `error/`: Defines custom error types and handling mechanisms.
  - `fault/`: Fault tolerance mechanisms, like circuit breakers.
  - `fl_core/`: Core components for federated learning processes.
  - `sharding/`: Logic for data sharding and partitioning, including Hilbert curve implementations.
  - `vector_db/`: Implementation of the vector database used for storing and querying embeddings.

- `federated/`: Implements the federated learning capabilities.
  - `metrics/`: Collectors for federated learning metrics.
  - `model/`: Management of model updates in the federated learning context.
  - `sync/`: Synchronization mechanisms for federated learning.

- `integration/`: Code for integrating with external systems or other parts of the Amazon Rose Forest ecosystem.
  - `yumechain/`: Specific integration with YumeiChan, likely related to AI agent interactions or data exchange.

- `knowledge/`: Modules related to knowledge representation and management.
  - `crdt/`: Conflict-free Replicated Data Types for distributed data consistency.
  - `representation/`: How knowledge is structured and stored.

- `metrics/`: General metrics collection and reporting for the system.
  - `collector.rs`: A common metrics collector.

- `query/`: Handles querying functionalities, possibly for the vector database or knowledge graph.
  - `router.rs`: Routes queries to the appropriate modules.

- `utils/`: Utility functions and common helper modules used across the `src` codebase.

- `lib.rs`: The root library crate, defining the public interface of the `src` library.
- `main.rs`: The main binary crate, serving as the entry point for the executable application.

## Development Guidelines

1. Code Standards
   - Follow consistent naming conventions (e.g., snake_case for functions and variables, PascalCase for types).
   - Write comprehensive documentation (module-level, function-level, and inline comments where necessary).
   - Include unit tests for new features and bug fixes.
   - Maintain code coverage standards.

2. Git Workflow
   - Create feature branches from `main` (or the relevant development branch).
   - Submit pull requests for review before merging.
   - Keep commits atomic and well-documented with clear messages.
   - Follow semantic versioning for releases.

3. Testing Requirements
   - Unit tests (`#[test]`) should accompany all new Rust functions and modules.
   - Integration tests should cover interactions between components.
   - Performance benchmarks for critical paths should be considered.
   - Security testing for sensitive features is crucial.

4. Documentation
   - Rust doc comments (`///` or `//!`) should be used extensively.
   - API documentation should be kept up-to-date using `cargo doc`.
   - Significant architecture changes should be documented in `docs/architecture/`.
   - Include examples for complex features or library usage.

## Building the Project

To build the project, ensure you have Rust and Cargo installed.

```bash
# Navigate to the project root
cd /path/to/amazon-rose-forest

# Build the project
cargo build

# Run tests
cargo test

# Run the application (if applicable)
cargo run
```

## Contributing

Contributions are welcome! Please follow these general steps:

1. Fork the repository.
2. Create a new feature branch from the `main` branch.
3. Make your changes, adhering to the development guidelines.
4. Commit your changes with clear and descriptive messages.
5. Push your branch to your fork.
6. Create a pull request against the `main` branch of the original repository.

## License

[To be determined]
The specific license for this project is yet to be finalized. Please refer to the `LICENSE` file in the root directory once it is available.
