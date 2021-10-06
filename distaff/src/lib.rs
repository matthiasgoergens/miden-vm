use air::{ProcessorAir, PublicInputs, TraceMetadata, TraceState, MAX_OUTPUTS, MIN_TRACE_LENGTH};
use core::convert::TryInto;
#[cfg(feature = "std")]
use log::debug;
use prover::{ExecutionTrace, ProverError, Serializable};
#[cfg(feature = "std")]
use std::time::Instant;

#[cfg(test)]
mod tests;

// EXPORTS
// ================================================================================================

pub use assembly;
pub use processor::{BaseElement, FieldElement, Program, ProgramInputs, StarkField};
pub use prover::{FieldExtension, HashFunction, ProofOptions, StarkProof};
pub use verifier::{verify, VerifierError};

// EXECUTOR
// ================================================================================================

/// Executes the specified `program` and returns the result together with a STARK-based proof of execution.
///
/// * `inputs` specifies the initial stack state and provides secret input tapes;
/// * `num_outputs` specifies the number of elements from the top of the stack to be returned;
pub fn execute(
    program: &Program,
    inputs: &ProgramInputs,
    num_outputs: usize,
    options: &ProofOptions,
) -> Result<(Vec<u128>, StarkProof), ProverError> {
    assert!(
        num_outputs <= MAX_OUTPUTS,
        "cannot produce more than {} outputs, but requested {}",
        MAX_OUTPUTS,
        num_outputs
    );

    // execute the program to create an execution trace
    #[cfg(feature = "std")]
    let now = Instant::now();
    let trace = processor::execute(program, inputs);
    #[cfg(feature = "std")]
    debug!(
        "Generated execution trace of {} registers and {} steps in {} ms",
        trace.width(),
        trace.length(),
        now.elapsed().as_millis()
    );

    // copy the user stack state the the last step to return as output
    let last_state = get_last_state(&trace);
    let outputs = last_state.user_stack()[..num_outputs]
        .iter()
        .map(|&v| v.as_int())
        .collect::<Vec<_>>();

    // make sure number of executed operations was sufficient
    assert!(
        last_state.op_counter().as_int() as usize >= MIN_TRACE_LENGTH,
        "a program must consist of at least {} operation, but only {} were executed",
        MIN_TRACE_LENGTH,
        last_state.op_counter()
    );

    // make sure program hash generated by the VM matches the hash of the program
    let program_hash: [u8; 32] = last_state.program_hash().to_bytes().try_into().unwrap();
    #[cfg(feature = "std")]
    assert!(
        *program.hash() == program_hash,
        "expected program hash {} does not match trace hash {}",
        hex::encode(program.hash()),
        hex::encode(program_hash)
    );

    // generate STARK proof
    let inputs = inputs
        .public_inputs()
        .iter()
        .map(|&v| v.as_int())
        .collect::<Vec<_>>();
    let pub_inputs = PublicInputs::new(program_hash, &inputs, &outputs);
    let proof = prover::prove::<ProcessorAir>(trace, pub_inputs, options.clone())?;

    Ok((outputs, proof))
}

// HELPER FUNCTIONS
// ================================================================================================

fn get_last_state(trace: &ExecutionTrace<BaseElement>) -> TraceState<BaseElement> {
    let last_step = trace.length() - 1;
    let meta = TraceMetadata::from_trace_info(&trace.get_info());

    let mut last_row = vec![BaseElement::ZERO; trace.width()];
    trace.read_row_into(last_step, &mut last_row);

    TraceState::from_slice(meta.ctx_depth, meta.loop_depth, meta.stack_depth, &last_row)
}

/// Prints out an execution trace.
#[allow(unused)]
fn print_trace(trace: &ExecutionTrace<BaseElement>, _multiples_of: usize) {
    let trace_width = trace.width();
    let meta = TraceMetadata::from_trace_info(&trace.get_info());

    let mut state = vec![BaseElement::ZERO; trace_width];
    for i in 0..trace.length() {
        trace.read_row_into(i, &mut state);
        let state =
            TraceState::from_slice(meta.ctx_depth, meta.loop_depth, meta.stack_depth, &state);
        println!("{:?}", state);
    }
}