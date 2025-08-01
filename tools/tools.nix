_: {
  perSystem =
    {
      pkgs,
      crane,
      ...
    }:
    {
      packages = {
        inherit (crane.buildWorkspaceMember "tools/devnet-utils" { }) devnet-utils;
        inherit (crane.buildWorkspaceMember "tools/update-deployments" { }) update-deployments;
        inherit (crane.buildWorkspaceMember "tools/build-evm-deployer-tx" { }) build-evm-deployer-tx;
        inherit (crane.buildWorkspaceMember "tools/json-schema-to-nixos-module-options" { })
          json-schema-to-nixos-module-options
          ;
        inherit
          (crane.buildWorkspaceMember "tools/u" {
            extraBuildInputs = [ pkgs.perl ];
            # clap-completions kinda sucks and is non-trivial to get to work nicely
            # extraNativeBuildInputs = [ pkgs.installShellFiles ];
            # postInstall = ''
            #   installShellCompletion --cmd u \
            #   --bash <($out/bin/u completions bash) \
            #   --fish <($out/bin/u completions fish) \
            #   --zsh <($out/bin/u completions zsh)
            # '';
          })
          u
          ;
        inherit (crane.buildWorkspaceMember "tools/rustfmt-sort" { }) rustfmt-sort;

        ignite-cli = pkgs.buildGoModule {
          name = "ignite-cli";
          src = pkgs.fetchFromGitHub {
            owner = "ignite";
            repo = "cli";
            rev = "v28.7.0";
            sha256 = "sha256-/gBykwBlZsHUWCJ01rdluU10xuEEmPmCfzSWlO6znW8=";
          };
          doCheck = false;
          vendorHash = "sha256-ks9wZUIwN0dOcXxxRk1Amxd0TPJBbLfKC9lzV4IMdjk=";
        };
      };
    };
}
