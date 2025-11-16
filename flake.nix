{
  description = "Strategy-based file balancing system for tiered storage with Tautulli integration";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        tierflow = pkgs.rustPlatform.buildRustPackage {
          pname = "tierflow";
          version = "0.1.5";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            rsync
          ];

          buildInputs = with pkgs; [
            openssl
          ];

          # Run tests during build
          doCheck = true;

          meta = with pkgs.lib; {
            description = "Strategy-based file balancing for tiered storage";
            homepage = "https://github.com/leonidbkh/tierflow";
            license = licenses.mit;
            maintainers = [ ];
            mainProgram = "tierflow";
          };
        };
      in
      {
        packages = {
          default = tierflow;
          tierflow = tierflow;
        };

        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustc
            cargo
            rust-analyzer
            rustfmt
            clippy
            pkg-config
            openssl
          ];

          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };

        # Allow running with `nix run`
        apps.default = {
          type = "app";
          program = "${tierflow}/bin/tierflow";
        };
      }
    ) // {
      # NixOS module
      nixosModules.default = { config, lib, pkgs, ... }:
        with lib;
        let
          cfg = config.services.tierflow;
          settingsFormat = pkgs.formats.yaml { };

          # Generate config file from settings
          configFile = if cfg.configFile != null
            then cfg.configFile
            else settingsFormat.generate "tierflow-config.yaml" cfg.settings;
        in
        {
          options.services.tierflow = {
            enable = mkEnableOption "Tierflow file balancing system";

            package = mkOption {
              type = types.package;
              default = self.packages.${pkgs.system}.default;
              defaultText = literalExpression "self.packages.\${pkgs.system}.default";
              description = "The tierflow package to use.";
            };

            interval = mkOption {
              type = types.int;
              default = 3600;
              description = "Interval in seconds between rebalancing runs (daemon mode).";
            };

            settings = mkOption {
              type = settingsFormat.type;
              default = { };
              description = ''
                Configuration for tierflow. This will be converted to YAML.
                See https://github.com/leonidbkh/tierflow for configuration details.
              '';
              example = literalExpression ''
                {
                  tiers = [
                    {
                      name = "cache";
                      path = "/mnt/nvme";
                      priority = 1;
                      max_usage_percent = 85;
                    }
                    {
                      name = "storage";
                      path = "/mnt/hdds";
                      priority = 10;
                    }
                  ];

                  strategies = [
                    {
                      name = "active_shows_on_cache";
                      priority = 100;
                      required = true;
                      conditions = [
                        {
                          type = "active_window";
                          days_back = 30;
                          backward_episodes = 2;
                          forward_episodes = 5;
                        }
                      ];
                      preferred_tiers = [ "cache" ];
                    }
                  ];

                  tautulli = {
                    enabled = true;
                    url = "http://localhost:8181";
                    api_key_file = "/run/secrets/tautulli-api-key";
                  };
                }
              '';
            };

            configFile = mkOption {
              type = types.nullOr types.path;
              default = null;
              description = ''
                Path to existing YAML configuration file.
                If set, this takes precedence over `settings`.
              '';
            };

            user = mkOption {
              type = types.str;
              default = "tierflow";
              description = "User account under which tierflow runs.";
            };

            group = mkOption {
              type = types.str;
              default = "tierflow";
              description = "Group under which tierflow runs.";
            };

            extraArgs = mkOption {
              type = types.listOf types.str;
              default = [ ];
              description = "Additional command-line arguments to pass to tierflow.";
              example = [ "-vv" ];
            };
          };

          config = mkIf cfg.enable {
            # Create user and group
            users.users.${cfg.user} = {
              isSystemUser = true;
              group = cfg.group;
              description = "Tierflow service user";
            };

            users.groups.${cfg.group} = { };

            # Systemd service
            systemd.services.tierflow = {
              description = "Tierflow file balancing daemon";
              after = [ "network.target" ];
              wantedBy = [ "multi-user.target" ];

              serviceConfig = {
                Type = "simple";
                User = cfg.user;
                Group = cfg.group;
                ExecStart = "${cfg.package}/bin/tierflow daemon --config ${configFile} --interval ${toString cfg.interval} ${concatStringsSep " " cfg.extraArgs}";
                Restart = "on-failure";
                RestartSec = "30s";

                # Security hardening
                NoNewPrivileges = true;
                PrivateTmp = true;
                ProtectSystem = "strict";
                ProtectHome = true;
                ReadWritePaths =
                  # Extract tier paths from settings for ReadWritePaths
                  if cfg.settings ? tiers then
                    map (tier: tier.path) cfg.settings.tiers
                  else [ ];

                # Logging
                StandardOutput = "journal";
                StandardError = "journal";
              };
            };
          };
        };
    };
}
