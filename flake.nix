{
  description = "Number games for a room of people";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    lib = nixpkgs.lib;
    systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
    forAllSystems = f: lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
  in {
    formatter = forAllSystems (pkgs: pkgs.alejandra);

    packages = forAllSystems (pkgs: {
      default = pkgs.rustPlatform.buildRustPackage {
        pname = "numbers";
        version = "0.1.0";
        src = lib.cleanSource self;
        cargoLock = {
          lockFile = ./Cargo.lock;
          # rooms is a git dependency, so nix needs its hash as well as
          # cargo's. Both change together: after bumping the rev in
          # Cargo.lock, run
          #   nix run nixpkgs#nix-prefetch-git -- --url https://github.com/float3/rooms --rev <rev>
          # and put the hash it prints here.
          outputHashes."rooms-0.1.0" = "sha256-pYDcX0k7+Jo6j+mCRh0m8lXfCwZBbyE+ULvjT6aEsXk=";
        };
        meta.mainProgram = "numbers";
      };
    });

    devShells = forAllSystems (pkgs: {
      default = pkgs.mkShell {
        packages = with pkgs; [cargo clippy rustc rustfmt];
      };
    });

    nixosModules.default = {
      config,
      lib,
      pkgs,
      ...
    }: let
      cfg = config.services.numbers;
    in {
      options.services.numbers = {
        enable = lib.mkEnableOption "the number games";
        package = lib.mkOption {
          type = lib.types.package;
          default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
        };
        port = lib.mkOption {
          type = lib.types.port;
          default = 8098;
        };
        domains = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [];
          description = "Serve through nginx with ACME on each of these domains.";
        };
      };

      config = lib.mkIf cfg.enable {
        systemd.services.numbers = {
          description = "Number games";
          wantedBy = ["multi-user.target"];
          after = ["network.target"];
          environment = {
            NUMBERS_ADDR = "127.0.0.1:${toString cfg.port}";
            NUMBERS_STATE = "/var/lib/numbers/rooms.json";
          };
          serviceConfig = {
            ExecStart = lib.getExe cfg.package;
            DynamicUser = true;
            StateDirectory = "numbers";
            Restart = "always";
            RestartSec = 1;
          };
        };

        services.nginx.virtualHosts = lib.genAttrs cfg.domains (domain: {
          forceSSL = true;
          enableACME = true;
          locations."/" = {
            proxyPass = "http://127.0.0.1:${toString cfg.port}";
            # Event streams: nothing to buffer and nothing to time out.
            extraConfig = ''
              proxy_buffering off;
              proxy_read_timeout 1h;
            '';
          };
        });
      };
    };
  };
}
