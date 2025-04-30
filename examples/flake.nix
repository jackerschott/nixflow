{
    inputs = {
        nixflow = {
            url = "github:jackerschott/nixflow/main";
            inputs.nixpkgs.follows = "nixpkgs";
        };
    };

    outputs = { self, nixpkgs, nixflow }:
    {
        packages.x86_64-linux.default = nixflow.lib.makeStepsPrinter ./rng.nix;
    };
}
