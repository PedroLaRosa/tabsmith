default:
    @just --list

# Compile a debug build
build:
    cargo build --locked

# Run the unit and integration tests
test:
    cargo test --locked

# Load every bundled spec module, as CI does
specs-test:
    cargo test --locked --release -- --ignored all_bundle_modules

# Build and install tabsmith into ~/.cargo/bin
install:
    cargo install --path . --locked

# Add the shell integration to your dotfiles (asks first)
setup:
    tabsmith setup

# Re-bundle @tabsmith/autocomplete-specs (defaults to assets/SPECS_VERSION)
update-specs version='':
    scripts/update-specs.sh {{ version }}

# Re-bundle the specs, check they load, then install the rebuilt binary
refresh-specs version='': (update-specs version) specs-test install
