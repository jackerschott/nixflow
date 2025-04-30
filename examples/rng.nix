{ pkgs, lib, nixflow, ... }:

let
    generate_random_integer = (i: rec {
        name = "generate_random_integer";
        outputs.integer = "integer_${toString i}.txt";
        run = nixflow.pythonScript { libraries = []; } ''
            from random import randint
            with open("${outputs.integer}", "w") as f:
                f.write(str(randint(0, 10)))
        '';
    });

    compute_mean = rec {
        name = "compute_mean";
        inputs.integers = map
            (i: nixflow.output (generate_random_integer i) "integer")
            (lib.genList (i: i) 10);

        outputs.mean = "mean.txt";
        run = nixflow.pythonScript {
            libraries = with pkgs; with pkgs.python312Packages; [numpy libz];
        } ''
            from numpy import mean

            input_paths = [
                ${lib.concatStringsSep ",\n    " (map (x: "\"${x.path}\"") inputs.integers)}
            ]
            integers = []
            for path in input_paths:
                with open(path) as f:
                    integers.append(int(f.read()))

            with open("${outputs.mean}", "w") as f:
                f.write(str(mean(integers)))
        '';
    };
in {
    mean = nixflow.output compute_mean "mean";
}
