{ self, pkgs, lib, ... }:

{
    preamble = {
        output = step: outputName: {
            path = step.outputs.${outputName};
            parentStep = step;
        };

        pythonScript = arguments: script: pkgs.writers.writePython3 "run" arguments script;
    };

    lib = {
        makeStepsPrinter = workflowStepsPath: (workflow: pkgs.writers.writeBashBin "steps" ''
            printf '%s' '${builtins.toJSON workflow}'
        '')(import workflowStepsPath { nixflow = self.preamble; inherit pkgs lib; });
    };
}
