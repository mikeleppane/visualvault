# List available commands
default:
    @just --list

all: format lint

# Format code using rustfmt
format:
    @echo "\nFormatting all code..."
    cargo fmt --all
    @echo "Done formatting!\n"

# Run clippy to lint the code
lint:
    @echo "\nLinting with clippy..."
    cargo fmt -- --check
    cargo clippy --all-features
    @echo "Done linting!\n"

# Fix linting issues where possible
lint-fix:
    @echo "Fixing linting issues..."
    cargo clippy --fix -- -D warnings