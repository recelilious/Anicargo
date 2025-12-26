# Repository Guidelines

## Project Status
This repository is currently empty (no source files or build scripts yet). Use this guide to establish a consistent structure and update it as the project grows.

## Project Structure & Module Organization
Create a clear top-level layout and keep new modules small and focused. A common baseline:

- `src/` for production code (group by feature or layer).
- `tests/` for test suites or integration tests.
- `assets/` for static files (images, fixtures, data).
- `docs/` for design notes or architecture decisions.

Document any deviations in `README.md` once you add code.

## Build, Test, and Development Commands
No build or test commands are defined yet. When you choose a stack, add a single entry point for each workflow and list it here. Example placeholders (replace with real commands):

- `make build` — compile or bundle the project.
- `make test` — run the full test suite.
- `make lint` — run static analysis or formatting checks.

## Coding Style & Naming Conventions
No formatter or linter is configured yet. When you add one, make it the source of truth and keep config in the repo root. Until then:

- Keep indentation consistent within a file (2 or 4 spaces).
- Use `snake_case` for filenames unless the language standard differs.
- Prefer explicit, descriptive module names (e.g., `order_parser`, `anime_catalog`).

## Testing Guidelines
No test framework is configured yet. When tests are added:

- Place new tests alongside code in `src/` or centrally in `tests/`.
- Use clear, behavior-focused names (e.g., `parses_valid_catalog`).
- Add tests for bug fixes and new features.

## Commit & Pull Request Guidelines
No git history is available to infer conventions. Until a standard is adopted:

- Use short, imperative commit messages (e.g., `init: add project scaffold`).
- Keep PRs focused, include a short description, and list tests run.
- Add screenshots for UI changes and link relevant issues.
