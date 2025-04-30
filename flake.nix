{
    description = "NixFlow workflow executor";

    inputs = {
        nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    };

    outputs = { self, nixpkgs }:
    let 
        pkgs = nixpkgs.legacyPackages.x86_64-linux;
    in import ./default.nix {
        inherit self pkgs;
        lib = nixpkgs.lib;
    };
}
