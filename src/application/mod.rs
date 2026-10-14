mod manager;
pub use manager::{DotfileManager, DotfileManagerError, DotfileManagerLoadError};

mod plan;
pub use plan::{Plan, PlanExecutionError, PlanExecutor};

mod apply;

mod inspection;
pub use inspection::{InspectionError, ManagedEntryInspector};
