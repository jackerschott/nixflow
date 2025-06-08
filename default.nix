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

        executors = {
            default = { id = "default"; };
            slurm = params: { id = "slurm"; } // params;
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
    in rec {
        makeStepsPrinterProfile = workflowSpecificationPath: profile:
            (workflow: pkgs.writers.writeBashBin "steps" '' jq . <<< '${builtins.toJSON workflow}' '')
            (import workflowSpecificationPath { nixflow = self.preamble; inherit pkgs lib profile; });

        makeStepsPrinter = workflowSpecificationPath: profiles: lib.pipe profiles [
            (profiles: map (profile: { ${profile} = makeStepsPrinterProfile workflowSpecificationPath profile; }) profiles)
            lib.mergeAttrsList
        ];

        makeStepRunnersProfile = workflowSpecificationPath: profile: {system}: lib.pipe
            (import workflowSpecificationPath { nixflow = self.preamble; inherit pkgs lib profile; }) [
            collectStepRunners
            (runners: lib.mapAttrs (name: runner: { type = "app"; program = "${runner}"; }) runners)
        ];

        makeStepRunners = workflowSpecificationPath: profiles: lib.pipe profiles [
            (profiles: map (profile: { ${profile} = makeStepRunnersProfile workflowSpecificationPath profile; }) profiles)
            lib.mergeAttrsList
        ];
    };
}
