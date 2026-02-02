//! Instruction data deserialization helpers

use bytemuck::Pod;
use pinocchio::program_error::ProgramError;
use pinocchio_log::log;

/// Parse instruction data as type T (unaligned read with copy)
///
/// Uses bytemuck for deserialization. Since instruction data may not be aligned
/// after stripping the discriminator byte, this performs an unaligned read
/// and returns an owned copy of the data.
///
/// Returns `ProgramError::InvalidInstructionData` if the data size doesn't match.
///
/// # Example
///
/// ```ignore
/// use panchor::prelude::*;
///
/// #[repr(C)]
/// #[derive(Clone, Copy, Pod, Zeroable)]
/// pub struct MyInstructionData {
///     pub amount: u64,
/// }
///
/// let args: MyInstructionData = parse_instruction_data(data)?;
/// ```
#[track_caller]
pub fn parse_instruction_data<T: Pod>(data: &[u8]) -> Result<T, ProgramError> {
    bytemuck::try_pod_read_unaligned(data).map_err(|e| {
        match e {
            bytemuck::PodCastError::SizeMismatch => {
                log!(
                    "parse error: size mismatch - got={}, expected={}",
                    data.len(),
                    core::mem::size_of::<T>()
                );
            }
            bytemuck::PodCastError::TargetAlignmentGreaterAndInputNotAligned
            | bytemuck::PodCastError::AlignmentMismatch => {
                log!(
                    "parse error: alignment - ptr={}, align={}",
                    data.as_ptr() as usize,
                    core::mem::align_of::<T>()
                );
            }
            bytemuck::PodCastError::OutputSliceWouldHaveSlop => {
                log!("parse error: output slice would have slop");
            }
        }
        ProgramError::InvalidInstructionData
    })
}
