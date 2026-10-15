//! Allocation-instrumented wrapper around the canonical chart admission harness.

#[path = "pipeline_workloads/allocator.rs"]
#[allow(dead_code)]
mod allocator;
#[path = "chart_workloads/measure.rs"]
#[allow(dead_code)]
mod measure;

fn main() -> Result<(), twinkle_charts::ChartError> {
    twinkle_ui::install_pipeline_allocator_counter(allocator::counts);
    measure::run_with_allocation_counter(allocator::counts)
}
