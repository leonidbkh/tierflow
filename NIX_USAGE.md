# Nix/NixOS Usage

## Quick Start

```bash
# Try without installing
nix run github:leonidbkh/tierflow -- --help

# Install to profile
nix profile install github:leonidbkh/tierflow
```

## NixOS Module

### 1. Add to flake inputs

```nix
{
  inputs.tierflow.url = "github:leonidbkh/tierflow";

  outputs = { nixpkgs, tierflow, ... }: {
    nixosConfigurations.yourhost = nixpkgs.lib.nixosSystem {
      modules = [
        tierflow.nixosModules.default
        ./configuration.nix
      ];
    };
  };
}
```

### 2. Configure in configuration.nix

```nix
{
  services.tierflow = {
    enable = true;
    interval = 3600;  # Run every hour

    settings = {
      tiers = [
        { name = "cache"; path = "/mnt/nvme"; priority = 1; max_usage_percent = 85; }
        { name = "storage"; path = "/mnt/hdds"; priority = 10; }
      ];

      strategies = [
        {
          name = "active_shows_on_cache";
          priority = 100;
          conditions = [ { type = "active_window"; days_back = 30; } ];
          preferred_tiers = [ "cache" ];
        }
        {
          name = "default";
          priority = 10;
          conditions = [ { type = "always_true"; } ];
          preferred_tiers = [ "cache" "storage" ];
        }
      ];

      tautulli = {
        enabled = true;
        url = "http://localhost:8181";
        api_key_file = "/run/secrets/tautulli-api-key";
      };
    };
  };
}
```

See `config.example.yaml` for all available options.

### 3. Managing secrets

```nix
# With sops-nix
services.tierflow.settings.tautulli.api_key_file = config.sops.secrets.tautulli.path;

# With agenix
services.tierflow.settings.tautulli.api_key_file = config.age.secrets.tautulli.path;

# Manual
services.tierflow.settings.tautulli.api_key_file = "/var/secrets/tautulli-api-key";
```

## Service Management

```bash
systemctl status tierflow
journalctl -u tierflow -f
```

## Development

```bash
nix develop
cargo build
cargo test
```
