use serde::{Deserialize, Serialize};
use sr_policy::Mount;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MountPlan {
    pub enabled: bool,
    pub mounts: Vec<MountPlanEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MountPlanEntry {
    pub source: String,
    pub target: String,
    pub read_only: bool,
}

pub struct MountPlanBuilder;

impl MountPlanBuilder {
    pub fn build(mounts: &[Mount]) -> MountPlan {
        let plan_mounts = mounts
            .iter()
            .map(|mount| MountPlanEntry {
                source: mount.source.clone(),
                target: mount.target.clone(),
                read_only: mount.read_only,
            })
            .collect::<Vec<_>>();
        MountPlan {
            enabled: true,
            mounts: plan_mounts,
        }
    }
}
