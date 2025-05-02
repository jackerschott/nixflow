{ self, pkgs, lib, ... }:

{
    preamble = {
        output = step: outputName: {
            path = step.outputs.${outputName};
            parentStep = step;
        };

        pythonScript = arguments: script: pkgs.writers.writePython3 "run" arguments script;
    };

    lib = let
        collectParentSteps = inputs: (builtins.attrValues (lib.mapAttrs (name: input: input.parentStep) inputs));
        collectStepRunners = runners: step: if step ? inputs
            then runners ++ [step.run] ++ (collectParentSteps step.inputs)
            else runners ++ [step.run];
    in {
        makeStepsPrinter = workflowStepsPath:
            (workflow: pkgs.writers.writeBashBin "steps" '' printf '%s' '${builtins.toJSON workflow}' '')
            (import workflowStepsPath { nixflow = self.preamble; inherit pkgs lib; });

        makeStepRunners = workflowStepsPath:
            (workflow: collectStepRunners [] workflow.parentStep)
            (import workflowStepsPath { nixflow = self.preamble; inherit pkgs lib; });
    };
}
