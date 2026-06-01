use derive_more::Display;

#[derive(Display)]
pub enum JobState {
    #[display("BootFail")]
    BootFail,
    #[display("Cancelled")]
    Cancelled,
    #[display("Completed")]
    Completed,
    #[display("Configuring")]
    Configuring,
    #[display("Completing")]
    Completing,
    #[display("Deadline")]
    Deadline,
    #[display("Failed")]
    Failed,
    #[display("NodeFail")]
    NodeFail,
    #[display("out of memory")]
    OutOfMemory,
    #[display(
        "pending due to {}",
        reason
            .as_ref()
            .map(|reason| format!("{reason}"))
            .unwrap_or("unknown reason (no reason assigned yet)".to_owned())
    )]
    Pending { reason: Option<PendingReason> },
    #[display("Preempted")]
    Preempted,
    #[display("Running")]
    Running,
    #[display("ResvDelHold")]
    ResvDelHold,
    #[display("RequeueFed")]
    RequeueFed,
    #[display("RequeueHold")]
    RequeueHold,
    #[display("Requeued")]
    Requeued,
    #[display("Resizing")]
    Resizing,
    #[display("Revoked")]
    Revoked,
    #[display("Signaling")]
    Signaling,
    #[display("SpecialExit")]
    SpecialExit,
    #[display("StageOut")]
    StageOut,
    #[display("Stopped")]
    Stopped,
    #[display("Suspended")]
    Suspended,
    #[display("Timeout")]
    Timeout,
}
impl JobState {
    pub fn from_polling_output(output: &str) -> Result<Self, String> {
        match output.split(" ").collect::<Vec<_>>().as_slice() {
            ["BF", "None"] => Ok(JobState::BootFail),
            ["CA", "None"] => Ok(JobState::Cancelled),
            ["CD", "None"] => Ok(JobState::Completed),
            ["CF", "None"] => Ok(JobState::Configuring),
            ["CG", "None"] => Ok(JobState::Completing),
            ["DL", "None"] => Ok(JobState::Deadline),
            ["F", "None"] => Ok(JobState::Failed),
            ["NF", "None"] => Ok(JobState::NodeFail),
            ["OOM", "None"] => Ok(JobState::OutOfMemory),
            ["PD", reason] if *reason != "None" => Ok(JobState::Pending {
                reason: PendingReason::from_polling_output(reason).map_err(|err| {
                    format!("encountered pending code (PD) with invalid reason\n{err}")
                })?,
            }),
            ["PR", "None"] => Ok(JobState::Preempted),
            ["R", "None"] => Ok(JobState::Running),
            ["RD", "None"] => Ok(JobState::ResvDelHold),
            ["RF", "None"] => Ok(JobState::RequeueFed),
            ["RH", "None"] => Ok(JobState::RequeueHold),
            ["RQ", "None"] => Ok(JobState::Requeued),
            ["RS", "None"] => Ok(JobState::Resizing),
            ["RV", "None"] => Ok(JobState::Revoked),
            ["SI", "None"] => Ok(JobState::Signaling),
            ["SE", "None"] => Ok(JobState::SpecialExit),
            ["SO", "None"] => Ok(JobState::StageOut),
            ["ST", "None"] => Ok(JobState::Stopped),
            ["S", "None"] => Ok(JobState::Suspended),
            ["TO", "None"] => Ok(JobState::Timeout),
            ["PD", "None"] => Err(String::from(
                "encountered pending code (PD) without a reason",
            )),
            [code, reason] if *reason != "None" => Err(format!(
                "encountered pending reason with a non-pending code ({code})"
            )),
            [code, _] => Err(format!("encountered invalid status code ({code})")),
            _ => Err(format!(
                "encountered `{output}` which does not match the expected format `<code> <reason>`"
            )),
        }
    }
}

#[derive(Display)]
pub enum PendingReason {
    #[display("accounting policy")]
    AccountingPolicy,
    #[display("account not allowed")]
    AccountNotAllowed,
    #[display("association group burst buffer")]
    AssocGrpBB,
    #[display("association group burst buffer minutes")]
    AssocGrpBBMinutes,
    #[display("association group burst buffer run minutes")]
    AssocGrpBBRunMinutes,
    #[display("association group billing")]
    AssocGrpBilling,
    #[display("association group billing minutes")]
    AssocGrpBillingMinutes,
    #[display("association group billing run minutes")]
    AssocGrpBillingRunMinutes,
    #[display("association group cpu limit")]
    AssocGrpCpuLimit,
    #[display("association group cpu minutes limit")]
    AssocGrpCPUMinutesLimit,
    #[display("association group cpu run minutes limit")]
    AssocGrpCPURunMinutesLimit,
    #[display("association group energy")]
    AssocGrpEnergy,
    #[display("association group energy minutes")]
    AssocGrpEnergyMinutes,
    #[display("association group energy run minutes")]
    AssocGrpEnergyRunMinutes,
    #[display("association group gres")]
    AssocGrpGRES,
    #[display("association group gres minutes")]
    AssocGrpGRESMinutes,
    #[display("association group gres run minutes")]
    AssocGrpGRESRunMinutes,
    #[display("association group jobs limit")]
    AssocGrpJobsLimit,
    #[display("association group license")]
    AssocGrpLicense,
    #[display("association group license minutes")]
    AssocGrpLicenseMinutes,
    #[display("association group license run minutes")]
    AssocGrpLicenseRunMinutes,
    #[display("association group mem limit")]
    AssocGrpMemLimit,
    #[display("association group mem minutes")]
    AssocGrpMemMinutes,
    #[display("association group mem run minutes")]
    AssocGrpMemRunMinutes,
    #[display("association group node limit")]
    AssocGrpNodeLimit,
    #[display("association group node minutes")]
    AssocGrpNodeMinutes,
    #[display("association group node run minutes")]
    AssocGrpNodeRunMinutes,
    #[display("association group submit jobs limit")]
    AssocGrpSubmitJobsLimit,
    #[display("association group unknown")]
    AssocGrpUnknown,
    #[display("association group unknown minutes")]
    AssocGrpUnknownMinutes,
    #[display("association group unknown run minutes")]
    AssocGrpUnknownRunMinutes,
    #[display("association group wall limit")]
    AssocGrpWallLimit,
    #[display("association max burst buffer minutes per job")]
    AssocMaxBBMinutesPerJob,
    #[display("association max burst buffer per job")]
    AssocMaxBBPerJob,
    #[display("association max burst buffer per node")]
    AssocMaxBBPerNode,
    #[display("association max billing minutes per job")]
    AssocMaxBillingMinutesPerJob,
    #[display("association max billing per job")]
    AssocMaxBillingPerJob,
    #[display("association max billing per node")]
    AssocMaxBillingPerNode,
    #[display("association max cpu minutes per job limit")]
    AssocMaxCpuMinutesPerJobLimit,
    #[display("association max cpu per job limit")]
    AssocMaxCpuPerJobLimit,
    #[display("association max cpu per node")]
    AssocMaxCpuPerNode,
    #[display("association max energy minutes per job")]
    AssocMaxEnergyMinutesPerJob,
    #[display("association max energy per job")]
    AssocMaxEnergyPerJob,
    #[display("association max energy per node")]
    AssocMaxEnergyPerNode,
    #[display("association max gres minutes per job")]
    AssocMaxGRESMinutesPerJob,
    #[display("association max gres per job")]
    AssocMaxGRESPerJob,
    #[display("association max gres per node")]
    AssocMaxGRESPerNode,
    #[display("association max jobs limit")]
    AssocMaxJobsLimit,
    #[display("association max license minutes per job")]
    AssocMaxLicenseMinutesPerJob,
    #[display("association max license per job")]
    AssocMaxLicensePerJob,
    #[display("association max mem minutes per job")]
    AssocMaxMemMinutesPerJob,
    #[display("association max mem per job")]
    AssocMaxMemPerJob,
    #[display("association max mem per node")]
    AssocMaxMemPerNode,
    #[display("association max node minutes per job")]
    AssocMaxNodeMinutesPerJob,
    #[display("association max node per job limit")]
    AssocMaxNodePerJobLimit,
    #[display("association max submit job limit")]
    AssocMaxSubmitJobLimit,
    #[display("association max unknown minutes per job")]
    AssocMaxUnknownMinutesPerJob,
    #[display("association max unknown per job")]
    AssocMaxUnknownPerJob,
    #[display("association max unknown per node")]
    AssocMaxUnknownPerNode,
    #[display("association max wall duration per job limit")]
    AssocMaxWallDurationPerJobLimit,
    #[display("association job limit")]
    AssociationJobLimit,
    #[display("association resource limit")]
    AssociationResourceLimit,
    #[display("association time limit")]
    AssociationTimeLimit,
    #[display("bad constraints")]
    BadConstraints,
    #[display("begin time")]
    BeginTime,
    #[display("burst buffer operation")]
    BurstBufferOperation,
    #[display("burst buffer resources")]
    BurstBufferResources,
    #[display("burst buffer stage in")]
    BurstBufferStageIn,
    #[display("cleaning")]
    Cleaning,
    #[display("dead line")]
    DeadLine,
    #[display("dependency")]
    Dependency,
    #[display("dependency never satisfied")]
    DependencyNeverSatisfied,
    #[display("federation job lock")]
    FedJobLock,
    #[display("inactive limit")]
    InactiveLimit,
    #[display("invalid account")]
    InvalidAccount,
    #[display("invalid qos")]
    InvalidQOS,
    #[display("job array task limit")]
    JobArrayTaskLimit,
    #[display("job held admin")]
    JobHeldAdmin,
    #[display("job held user")]
    JobHeldUser,
    #[display("job hold max requeue")]
    JobHoldMaxRequeue,
    #[display("job launch failure")]
    JobLaunchFailure,
    #[display("licenses")]
    Licenses,
    #[display("max burst buffer per account")]
    MaxBBPerAccount,
    #[display("max billing per account")]
    MaxBillingPerAccount,
    #[display("max cpu per account")]
    MaxCpuPerAccount,
    #[display("max energy per account")]
    MaxEnergyPerAccount,
    #[display("max gres per account")]
    MaxGRESPerAccount,
    #[display("max jobs per account")]
    MaxJobsPerAccount,
    #[display("max license per account")]
    MaxLicensePerAccount,
    #[display("max memory per account")]
    MaxMemoryPerAccount,
    #[display("max mem per limit")]
    MaxMemPerLimit,
    #[display("max node per account")]
    MaxNodePerAccount,
    #[display("max submit jobs per account")]
    MaxSubmitJobsPerAccount,
    #[display("max unknown per account")]
    MaxUnknownPerAccount,
    #[display("node down")]
    NodeDown,
    #[display("non zero exit code")]
    NonZeroExitCode,
    #[display("out of memory")]
    OutOfMemory,
    #[display("partition config")]
    PartitionConfig,
    #[display("partition down")]
    PartitionDown,
    #[display("partition inactive")]
    PartitionInactive,
    #[display("partition node limit")]
    PartitionNodeLimit,
    #[display("partition time limit")]
    PartitionTimeLimit,
    #[display("priority")]
    Priority,
    #[display("prolog")]
    Prolog,
    #[display("qos group burst buffer")]
    QOSGrpBB,
    #[display("qos group burst buffer minutes")]
    QOSGrpBBMinutes,
    #[display("qos group burst buffer run minutes")]
    QOSGrpBBRunMinutes,
    #[display("qos group billing")]
    QOSGrpBilling,
    #[display("qos group billing minutes")]
    QOSGrpBillingMinutes,
    #[display("qos group billing run minutes")]
    QOSGrpBillingRunMinutes,
    #[display("qos group cpu limit")]
    QOSGrpCpuLimit,
    #[display("qos group cpu minutes limit")]
    QOSGrpCPUMinutesLimit,
    #[display("qos group cpu run minutes limit")]
    QOSGrpCPURunMinutesLimit,
    #[display("qos group energy")]
    QOSGrpEnergy,
    #[display("qos group energy minutes")]
    QOSGrpEnergyMinutes,
    #[display("qos group energy run minutes")]
    QOSGrpEnergyRunMinutes,
    #[display("qos group gres")]
    QOSGrpGRES,
    #[display("qos group gres minutes")]
    QOSGrpGRESMinutes,
    #[display("qos group gres run minutes")]
    QOSGrpGRESRunMinutes,
    #[display("qos group jobs limit")]
    QOSGrpJobsLimit,
    #[display("qos group license")]
    QOSGrpLicense,
    #[display("qos group license minutes")]
    QOSGrpLicenseMinutes,
    #[display("qos group license run minutes")]
    QOSGrpLicenseRunMinutes,
    #[display("qos group mem limit")]
    QOSGrpMemLimit,
    #[display("qos group memory minutes")]
    QOSGrpMemoryMinutes,
    #[display("qos group memory run minutes")]
    QOSGrpMemoryRunMinutes,
    #[display("qos group node limit")]
    QOSGrpNodeLimit,
    #[display("qos group node minutes")]
    QOSGrpNodeMinutes,
    #[display("qos group node run minutes")]
    QOSGrpNodeRunMinutes,
    #[display("qos group submit jobs limit")]
    QOSGrpSubmitJobsLimit,
    #[display("qos group unknown")]
    QOSGrpUnknown,
    #[display("qos group unknown minutes")]
    QOSGrpUnknownMinutes,
    #[display("qos group unknown run minutes")]
    QOSGrpUnknownRunMinutes,
    #[display("qos group wall limit")]
    QOSGrpWallLimit,
    #[display("qos job limit")]
    QOSJobLimit,
    #[display("qos max burst buffer minutes per job")]
    QOSMaxBBMinutesPerJob,
    #[display("qos max burst buffer per job")]
    QOSMaxBBPerJob,
    #[display("qos max burst buffer per node")]
    QOSMaxBBPerNode,
    #[display("qos max burst buffer per user")]
    QOSMaxBBPerUser,
    #[display("qos max billing minutes per job")]
    QOSMaxBillingMinutesPerJob,
    #[display("qos max billing per job")]
    QOSMaxBillingPerJob,
    #[display("qos max billing per node")]
    QOSMaxBillingPerNode,
    #[display("qos max billing per user")]
    QOSMaxBillingPerUser,
    #[display("qos max cpu minutes per job limit")]
    QOSMaxCpuMinutesPerJobLimit,
    #[display("qos max cpu per job limit")]
    QOSMaxCpuPerJobLimit,
    #[display("qos max cpu per node")]
    QOSMaxCpuPerNode,
    #[display("qos max cpu per user limit")]
    QOSMaxCpuPerUserLimit,
    #[display("qos max energy minutes per job")]
    QOSMaxEnergyMinutesPerJob,
    #[display("qos max energy per job")]
    QOSMaxEnergyPerJob,
    #[display("qos max energy per node")]
    QOSMaxEnergyPerNode,
    #[display("qos max energy per user")]
    QOSMaxEnergyPerUser,
    #[display("qos max gres minutes per job")]
    QOSMaxGRESMinutesPerJob,
    #[display("qos max gres per job")]
    QOSMaxGRESPerJob,
    #[display("qos max gres per node")]
    QOSMaxGRESPerNode,
    #[display("qos max gres per user")]
    QOSMaxGRESPerUser,
    #[display("qos max jobs per user limit")]
    QOSMaxJobsPerUserLimit,
    #[display("qos max license minutes per job")]
    QOSMaxLicenseMinutesPerJob,
    #[display("qos max license per job")]
    QOSMaxLicensePerJob,
    #[display("qos max license per user")]
    QOSMaxLicensePerUser,
    #[display("qos max memory minutes per job")]
    QOSMaxMemoryMinutesPerJob,
    #[display("qos max memory per job")]
    QOSMaxMemoryPerJob,
    #[display("qos max memory per node")]
    QOSMaxMemoryPerNode,
    #[display("qos max memory per user")]
    QOSMaxMemoryPerUser,
    #[display("qos max node minutes per job")]
    QOSMaxNodeMinutesPerJob,
    #[display("qos max node per job limit")]
    QOSMaxNodePerJobLimit,
    #[display("qos max node per user limit")]
    QOSMaxNodePerUserLimit,
    #[display("qos max submit job per user limit")]
    QOSMaxSubmitJobPerUserLimit,
    #[display("qos max unknown minutes per job")]
    QOSMaxUnknownMinutesPerJob,
    #[display("qos max unknown per job")]
    QOSMaxUnknownPerJob,
    #[display("qos max unknown per node")]
    QOSMaxUnknownPerNode,
    #[display("qos max unknown per user")]
    QOSMaxUnknownPerUser,
    #[display("qos max wall duration per job limit")]
    QOSMaxWallDurationPerJobLimit,
    #[display("qos min burst buffer")]
    QOSMinBB,
    #[display("qos min billing")]
    QOSMinBilling,
    #[display("qos min cpu not satisfied")]
    QOSMinCpuNotSatisfied,
    #[display("qos min energy")]
    QOSMinEnergy,
    #[display("qos min gres")]
    QOSMinGRES,
    #[display("qos min license")]
    QOSMinLicense,
    #[display("qos min memory")]
    QOSMinMemory,
    #[display("qos min node")]
    QOSMinNode,
    #[display("qos min unknown")]
    QOSMinUnknown,
    #[display("qos not allowed")]
    QOSNotAllowed,
    #[display("qos resource limit")]
    QOSResourceLimit,
    #[display("qos time limit")]
    QOSTimeLimit,
    #[display("qos usage threshold")]
    QOSUsageThreshold,
    #[display("required node not available")]
    ReqNodeNotAvail,
    #[display("reservation")]
    Reservation,
    #[display("reservation deleted")]
    ReservationDeleted,
    #[display("resources")]
    Resources,
    #[display("scheduler defer")]
    SchedDefer,
    #[display("system failure")]
    SystemFailure,
    #[display("time limit")]
    TimeLimit,
}
impl PendingReason {
    pub fn from_polling_output(output: &str) -> Result<Option<Self>, String> {
        match output {
            "AccountingPolicy" => Ok(Some(Self::AccountingPolicy)),
            "AccountNotAllowed" => Ok(Some(Self::AccountNotAllowed)),
            "AssocGrpBB" => Ok(Some(Self::AssocGrpBB)),
            "AssocGrpBBMinutes" => Ok(Some(Self::AssocGrpBBMinutes)),
            "AssocGrpBBRunMinutes" => Ok(Some(Self::AssocGrpBBRunMinutes)),
            "AssocGrpBilling" => Ok(Some(Self::AssocGrpBilling)),
            "AssocGrpBillingMinutes" => Ok(Some(Self::AssocGrpBillingMinutes)),
            "AssocGrpBillingRunMinutes" => Ok(Some(Self::AssocGrpBillingRunMinutes)),
            "AssocGrpCpuLimit" => Ok(Some(Self::AssocGrpCpuLimit)),
            "AssocGrpCPUMinutesLimit" => Ok(Some(Self::AssocGrpCPUMinutesLimit)),
            "AssocGrpCPURunMinutesLimit" => Ok(Some(Self::AssocGrpCPURunMinutesLimit)),
            "AssocGrpEnergy" => Ok(Some(Self::AssocGrpEnergy)),
            "AssocGrpEnergyMinutes" => Ok(Some(Self::AssocGrpEnergyMinutes)),
            "AssocGrpEnergyRunMinutes" => Ok(Some(Self::AssocGrpEnergyRunMinutes)),
            "AssocGrpGRES" => Ok(Some(Self::AssocGrpGRES)),
            "AssocGrpGRESMinutes" => Ok(Some(Self::AssocGrpGRESMinutes)),
            "AssocGrpGRESRunMinutes" => Ok(Some(Self::AssocGrpGRESRunMinutes)),
            "AssocGrpJobsLimit" => Ok(Some(Self::AssocGrpJobsLimit)),
            "AssocGrpLicense" => Ok(Some(Self::AssocGrpLicense)),
            "AssocGrpLicenseMinutes" => Ok(Some(Self::AssocGrpLicenseMinutes)),
            "AssocGrpLicenseRunMinutes" => Ok(Some(Self::AssocGrpLicenseRunMinutes)),
            "AssocGrpMemLimit" => Ok(Some(Self::AssocGrpMemLimit)),
            "AssocGrpMemMinutes" => Ok(Some(Self::AssocGrpMemMinutes)),
            "AssocGrpMemRunMinutes" => Ok(Some(Self::AssocGrpMemRunMinutes)),
            "AssocGrpNodeLimit" => Ok(Some(Self::AssocGrpNodeLimit)),
            "AssocGrpNodeMinutes" => Ok(Some(Self::AssocGrpNodeMinutes)),
            "AssocGrpNodeRunMinutes" => Ok(Some(Self::AssocGrpNodeRunMinutes)),
            "AssocGrpSubmitJobsLimit" => Ok(Some(Self::AssocGrpSubmitJobsLimit)),
            "AssocGrpUnknown" => Ok(Some(Self::AssocGrpUnknown)),
            "AssocGrpUnknownMinutes" => Ok(Some(Self::AssocGrpUnknownMinutes)),
            "AssocGrpUnknownRunMinutes" => Ok(Some(Self::AssocGrpUnknownRunMinutes)),
            "AssocGrpWallLimit" => Ok(Some(Self::AssocGrpWallLimit)),
            "AssocMaxBBMinutesPerJob" => Ok(Some(Self::AssocMaxBBMinutesPerJob)),
            "AssocMaxBBPerJob" => Ok(Some(Self::AssocMaxBBPerJob)),
            "AssocMaxBBPerNode" => Ok(Some(Self::AssocMaxBBPerNode)),
            "AssocMaxBillingMinutesPerJob" => Ok(Some(Self::AssocMaxBillingMinutesPerJob)),
            "AssocMaxBillingPerJob" => Ok(Some(Self::AssocMaxBillingPerJob)),
            "AssocMaxBillingPerNode" => Ok(Some(Self::AssocMaxBillingPerNode)),
            "AssocMaxCpuMinutesPerJobLimit" => Ok(Some(Self::AssocMaxCpuMinutesPerJobLimit)),
            "AssocMaxCpuPerJobLimit" => Ok(Some(Self::AssocMaxCpuPerJobLimit)),
            "AssocMaxCpuPerNode" => Ok(Some(Self::AssocMaxCpuPerNode)),
            "AssocMaxEnergyMinutesPerJob" => Ok(Some(Self::AssocMaxEnergyMinutesPerJob)),
            "AssocMaxEnergyPerJob" => Ok(Some(Self::AssocMaxEnergyPerJob)),
            "AssocMaxEnergyPerNode" => Ok(Some(Self::AssocMaxEnergyPerNode)),
            "AssocMaxGRESMinutesPerJob" => Ok(Some(Self::AssocMaxGRESMinutesPerJob)),
            "AssocMaxGRESPerJob" => Ok(Some(Self::AssocMaxGRESPerJob)),
            "AssocMaxGRESPerNode" => Ok(Some(Self::AssocMaxGRESPerNode)),
            "AssocMaxJobsLimit" => Ok(Some(Self::AssocMaxJobsLimit)),
            "AssocMaxLicenseMinutesPerJob" => Ok(Some(Self::AssocMaxLicenseMinutesPerJob)),
            "AssocMaxLicensePerJob" => Ok(Some(Self::AssocMaxLicensePerJob)),
            "AssocMaxMemMinutesPerJob" => Ok(Some(Self::AssocMaxMemMinutesPerJob)),
            "AssocMaxMemPerJob" => Ok(Some(Self::AssocMaxMemPerJob)),
            "AssocMaxMemPerNode" => Ok(Some(Self::AssocMaxMemPerNode)),
            "AssocMaxNodeMinutesPerJob" => Ok(Some(Self::AssocMaxNodeMinutesPerJob)),
            "AssocMaxNodePerJobLimit" => Ok(Some(Self::AssocMaxNodePerJobLimit)),
            "AssocMaxSubmitJobLimit" => Ok(Some(Self::AssocMaxSubmitJobLimit)),
            "AssocMaxUnknownMinutesPerJob" => Ok(Some(Self::AssocMaxUnknownMinutesPerJob)),
            "AssocMaxUnknownPerJob" => Ok(Some(Self::AssocMaxUnknownPerJob)),
            "AssocMaxUnknownPerNode" => Ok(Some(Self::AssocMaxUnknownPerNode)),
            "AssocMaxWallDurationPerJobLimit" => Ok(Some(Self::AssocMaxWallDurationPerJobLimit)),
            "AssociationJobLimit" => Ok(Some(Self::AssociationJobLimit)),
            "AssociationResourceLimit" => Ok(Some(Self::AssociationResourceLimit)),
            "AssociationTimeLimit" => Ok(Some(Self::AssociationTimeLimit)),
            "BadConstraints" => Ok(Some(Self::BadConstraints)),
            "BeginTime" => Ok(Some(Self::BeginTime)),
            "BurstBufferOperation" => Ok(Some(Self::BurstBufferOperation)),
            "BurstBufferResources" => Ok(Some(Self::BurstBufferResources)),
            "BurstBufferStageIn" => Ok(Some(Self::BurstBufferStageIn)),
            "Cleaning" => Ok(Some(Self::Cleaning)),
            "DeadLine" => Ok(Some(Self::DeadLine)),
            "Dependency" => Ok(Some(Self::Dependency)),
            "DependencyNeverSatisfied" => Ok(Some(Self::DependencyNeverSatisfied)),
            "FedJobLock" => Ok(Some(Self::FedJobLock)),
            "InactiveLimit" => Ok(Some(Self::InactiveLimit)),
            "InvalidAccount" => Ok(Some(Self::InvalidAccount)),
            "InvalidQOS" => Ok(Some(Self::InvalidQOS)),
            "JobArrayTaskLimit" => Ok(Some(Self::JobArrayTaskLimit)),
            "JobHeldAdmin" => Ok(Some(Self::JobHeldAdmin)),
            "JobHeldUser" => Ok(Some(Self::JobHeldUser)),
            "JobHoldMaxRequeue" => Ok(Some(Self::JobHoldMaxRequeue)),
            "JobLaunchFailure" => Ok(Some(Self::JobLaunchFailure)),
            "Licenses" => Ok(Some(Self::Licenses)),
            "MaxBBPerAccount" => Ok(Some(Self::MaxBBPerAccount)),
            "MaxBillingPerAccount" => Ok(Some(Self::MaxBillingPerAccount)),
            "MaxCpuPerAccount" => Ok(Some(Self::MaxCpuPerAccount)),
            "MaxEnergyPerAccount" => Ok(Some(Self::MaxEnergyPerAccount)),
            "MaxGRESPerAccount" => Ok(Some(Self::MaxGRESPerAccount)),
            "MaxJobsPerAccount" => Ok(Some(Self::MaxJobsPerAccount)),
            "MaxLicensePerAccount" => Ok(Some(Self::MaxLicensePerAccount)),
            "MaxMemoryPerAccount" => Ok(Some(Self::MaxMemoryPerAccount)),
            "MaxMemPerLimit" => Ok(Some(Self::MaxMemPerLimit)),
            "MaxNodePerAccount" => Ok(Some(Self::MaxNodePerAccount)),
            "MaxSubmitJobsPerAccount" => Ok(Some(Self::MaxSubmitJobsPerAccount)),
            "MaxUnknownPerAccount" => Ok(Some(Self::MaxUnknownPerAccount)),
            "NodeDown" => Ok(Some(Self::NodeDown)),
            "NonZeroExitCode" => Ok(Some(Self::NonZeroExitCode)),
            "OutOfMemory" => Ok(Some(Self::OutOfMemory)),
            "PartitionConfig" => Ok(Some(Self::PartitionConfig)),
            "PartitionDown" => Ok(Some(Self::PartitionDown)),
            "PartitionInactive" => Ok(Some(Self::PartitionInactive)),
            "PartitionNodeLimit" => Ok(Some(Self::PartitionNodeLimit)),
            "PartitionTimeLimit" => Ok(Some(Self::PartitionTimeLimit)),
            "Priority" => Ok(Some(Self::Priority)),
            "Prolog" => Ok(Some(Self::Prolog)),
            "QOSGrpBB" => Ok(Some(Self::QOSGrpBB)),
            "QOSGrpBBMinutes" => Ok(Some(Self::QOSGrpBBMinutes)),
            "QOSGrpBBRunMinutes" => Ok(Some(Self::QOSGrpBBRunMinutes)),
            "QOSGrpBilling" => Ok(Some(Self::QOSGrpBilling)),
            "QOSGrpBillingMinutes" => Ok(Some(Self::QOSGrpBillingMinutes)),
            "QOSGrpBillingRunMinutes" => Ok(Some(Self::QOSGrpBillingRunMinutes)),
            "QOSGrpCpuLimit" => Ok(Some(Self::QOSGrpCpuLimit)),
            "QOSGrpCPUMinutesLimit" => Ok(Some(Self::QOSGrpCPUMinutesLimit)),
            "QOSGrpCPURunMinutesLimit" => Ok(Some(Self::QOSGrpCPURunMinutesLimit)),
            "QOSGrpEnergy" => Ok(Some(Self::QOSGrpEnergy)),
            "QOSGrpEnergyMinutes" => Ok(Some(Self::QOSGrpEnergyMinutes)),
            "QOSGrpEnergyRunMinutes" => Ok(Some(Self::QOSGrpEnergyRunMinutes)),
            "QOSGrpGRES" => Ok(Some(Self::QOSGrpGRES)),
            "QOSGrpGRESMinutes" => Ok(Some(Self::QOSGrpGRESMinutes)),
            "QOSGrpGRESRunMinutes" => Ok(Some(Self::QOSGrpGRESRunMinutes)),
            "QOSGrpJobsLimit" => Ok(Some(Self::QOSGrpJobsLimit)),
            "QOSGrpLicense" => Ok(Some(Self::QOSGrpLicense)),
            "QOSGrpLicenseMinutes" => Ok(Some(Self::QOSGrpLicenseMinutes)),
            "QOSGrpLicenseRunMinutes" => Ok(Some(Self::QOSGrpLicenseRunMinutes)),
            "QOSGrpMemLimit" => Ok(Some(Self::QOSGrpMemLimit)),
            "QOSGrpMemoryMinutes" => Ok(Some(Self::QOSGrpMemoryMinutes)),
            "QOSGrpMemoryRunMinutes" => Ok(Some(Self::QOSGrpMemoryRunMinutes)),
            "QOSGrpNodeLimit" => Ok(Some(Self::QOSGrpNodeLimit)),
            "QOSGrpNodeMinutes" => Ok(Some(Self::QOSGrpNodeMinutes)),
            "QOSGrpNodeRunMinutes" => Ok(Some(Self::QOSGrpNodeRunMinutes)),
            "QOSGrpSubmitJobsLimit" => Ok(Some(Self::QOSGrpSubmitJobsLimit)),
            "QOSGrpUnknown" => Ok(Some(Self::QOSGrpUnknown)),
            "QOSGrpUnknownMinutes" => Ok(Some(Self::QOSGrpUnknownMinutes)),
            "QOSGrpUnknownRunMinutes" => Ok(Some(Self::QOSGrpUnknownRunMinutes)),
            "QOSGrpWallLimit" => Ok(Some(Self::QOSGrpWallLimit)),
            "QOSJobLimit" => Ok(Some(Self::QOSJobLimit)),
            "QOSMaxBBMinutesPerJob" => Ok(Some(Self::QOSMaxBBMinutesPerJob)),
            "QOSMaxBBPerJob" => Ok(Some(Self::QOSMaxBBPerJob)),
            "QOSMaxBBPerNode" => Ok(Some(Self::QOSMaxBBPerNode)),
            "QOSMaxBBPerUser" => Ok(Some(Self::QOSMaxBBPerUser)),
            "QOSMaxBillingMinutesPerJob" => Ok(Some(Self::QOSMaxBillingMinutesPerJob)),
            "QOSMaxBillingPerJob" => Ok(Some(Self::QOSMaxBillingPerJob)),
            "QOSMaxBillingPerNode" => Ok(Some(Self::QOSMaxBillingPerNode)),
            "QOSMaxBillingPerUser" => Ok(Some(Self::QOSMaxBillingPerUser)),
            "QOSMaxCpuMinutesPerJobLimit" => Ok(Some(Self::QOSMaxCpuMinutesPerJobLimit)),
            "QOSMaxCpuPerJobLimit" => Ok(Some(Self::QOSMaxCpuPerJobLimit)),
            "QOSMaxCpuPerNode" => Ok(Some(Self::QOSMaxCpuPerNode)),
            "QOSMaxCpuPerUserLimit" => Ok(Some(Self::QOSMaxCpuPerUserLimit)),
            "QOSMaxEnergyMinutesPerJob" => Ok(Some(Self::QOSMaxEnergyMinutesPerJob)),
            "QOSMaxEnergyPerJob" => Ok(Some(Self::QOSMaxEnergyPerJob)),
            "QOSMaxEnergyPerNode" => Ok(Some(Self::QOSMaxEnergyPerNode)),
            "QOSMaxEnergyPerUser" => Ok(Some(Self::QOSMaxEnergyPerUser)),
            "QOSMaxGRESMinutesPerJob" => Ok(Some(Self::QOSMaxGRESMinutesPerJob)),
            "QOSMaxGRESPerJob" => Ok(Some(Self::QOSMaxGRESPerJob)),
            "QOSMaxGRESPerNode" => Ok(Some(Self::QOSMaxGRESPerNode)),
            "QOSMaxGRESPerUser" => Ok(Some(Self::QOSMaxGRESPerUser)),
            "QOSMaxJobsPerUserLimit" => Ok(Some(Self::QOSMaxJobsPerUserLimit)),
            "QOSMaxLicenseMinutesPerJob" => Ok(Some(Self::QOSMaxLicenseMinutesPerJob)),
            "QOSMaxLicensePerJob" => Ok(Some(Self::QOSMaxLicensePerJob)),
            "QOSMaxLicensePerUser" => Ok(Some(Self::QOSMaxLicensePerUser)),
            "QOSMaxMemoryMinutesPerJob" => Ok(Some(Self::QOSMaxMemoryMinutesPerJob)),
            "QOSMaxMemoryPerJob" => Ok(Some(Self::QOSMaxMemoryPerJob)),
            "QOSMaxMemoryPerNode" => Ok(Some(Self::QOSMaxMemoryPerNode)),
            "QOSMaxMemoryPerUser" => Ok(Some(Self::QOSMaxMemoryPerUser)),
            "QOSMaxNodeMinutesPerJob" => Ok(Some(Self::QOSMaxNodeMinutesPerJob)),
            "QOSMaxNodePerJobLimit" => Ok(Some(Self::QOSMaxNodePerJobLimit)),
            "QOSMaxNodePerUserLimit" => Ok(Some(Self::QOSMaxNodePerUserLimit)),
            "QOSMaxSubmitJobPerUserLimit" => Ok(Some(Self::QOSMaxSubmitJobPerUserLimit)),
            "QOSMaxUnknownMinutesPerJob" => Ok(Some(Self::QOSMaxUnknownMinutesPerJob)),
            "QOSMaxUnknownPerJob" => Ok(Some(Self::QOSMaxUnknownPerJob)),
            "QOSMaxUnknownPerNode" => Ok(Some(Self::QOSMaxUnknownPerNode)),
            "QOSMaxUnknownPerUser" => Ok(Some(Self::QOSMaxUnknownPerUser)),
            "QOSMaxWallDurationPerJobLimit" => Ok(Some(Self::QOSMaxWallDurationPerJobLimit)),
            "QOSMinBB" => Ok(Some(Self::QOSMinBB)),
            "QOSMinBilling" => Ok(Some(Self::QOSMinBilling)),
            "QOSMinCpuNotSatisfied" => Ok(Some(Self::QOSMinCpuNotSatisfied)),
            "QOSMinEnergy" => Ok(Some(Self::QOSMinEnergy)),
            "QOSMinGRES" => Ok(Some(Self::QOSMinGRES)),
            "QOSMinLicense" => Ok(Some(Self::QOSMinLicense)),
            "QOSMinMemory" => Ok(Some(Self::QOSMinMemory)),
            "QOSMinNode" => Ok(Some(Self::QOSMinNode)),
            "QOSMinUnknown" => Ok(Some(Self::QOSMinUnknown)),
            "QOSNotAllowed" => Ok(Some(Self::QOSNotAllowed)),
            "QOSResourceLimit" => Ok(Some(Self::QOSResourceLimit)),
            "QOSTimeLimit" => Ok(Some(Self::QOSTimeLimit)),
            "QOSUsageThreshold" => Ok(Some(Self::QOSUsageThreshold)),
            "ReqNodeNotAvail" => Ok(Some(Self::ReqNodeNotAvail)),
            "Reservation" => Ok(Some(Self::Reservation)),
            "ReservationDeleted" => Ok(Some(Self::ReservationDeleted)),
            "Resources" => Ok(Some(Self::Resources)),
            "SchedDefer" => Ok(Some(Self::SchedDefer)),
            "SystemFailure" => Ok(Some(Self::SystemFailure)),
            "TimeLimit" => Ok(Some(Self::TimeLimit)),
            "None" => Ok(None),
            _ => Err(format!("encountered invalid pending reason `{output}`")),
        }
    }
}
