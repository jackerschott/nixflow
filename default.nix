{ self, pkgs, lib, ... }:

let
    singleOrListToList = singleOrListToList: if builtins.isList singleOrListToList
        then singleOrListToList
        else [singleOrListToList];
in {
    preamble = {
        output = step: outputName: {
            path = step.outputs.${outputName};
            parentStep = step;
        };

        pythonScript = arguments: script: pkgs.writers.writePython3 "run" arguments script;
    };

    lib = let
        collectParentSteps = outputs: lib.concatLists (builtins.attrValues (lib.mapAttrs
            (name: inputs: map (input: input.parentStep) (singleOrListToList inputs))
            outputs));
        collectStepRunners = outputs: lib.pipe outputs [
            collectParentSteps
            (steps: map (step: if step ? inputs
                then { ${step.name} = step.run; } // (collectStepRunners step.inputs)
                else { ${step.name} = step.run; }) steps)
            lib.mergeAttrsList
        ];
    in {
        makeStepsPrinter = workflowSpecificationPath:
            (workflow: pkgs.writers.writeBashBin "steps" '' printf '%s' '${builtins.toJSON workflow}' '')
            (import workflowSpecificationPath { nixflow = self.preamble; inherit pkgs lib; });

        makeStepRunners = workflowSpecificationPath: {system}: lib.pipe
            (import workflowSpecificationPath { nixflow = self.preamble; inherit pkgs lib; }) [
            collectStepRunners
            (runners: lib.mapAttrs (name: runner: { type = "app"; program = "${runner}"; }) runners)
        ];
    };
}
