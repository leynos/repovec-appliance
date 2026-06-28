//! Debug formatting for device-flow runtime adapter bundles.

use std::fmt;

use super::{DeviceFlowApi, DeviceFlowRuntime, Sleeper, TokenStore};

impl<A, T, S> fmt::Debug for DeviceFlowRuntime<'_, A, T, S>
where
    A: DeviceFlowApi + fmt::Debug,
    T: TokenStore + fmt::Debug,
    S: Sleeper + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeviceFlowRuntime")
            .field("api", self.api)
            .field("store", self.store)
            .field("sleeper", self.sleeper)
            .field("clock", &"<monotonic clock>")
            .finish()
    }
}
