use std::error::Error;
use systems_wasm::aspects::{EventToVariableMapping, MsfsAspectBuilder};
use systems_wasm::Variable;

pub(super) fn circuit_brakers(builder: &mut MsfsAspectBuilder) -> Result<(), Box<dyn Error>> {
    for panel_name in ["Name1", "Name2"] {
        let variable = Variable::aspect(&format!("CB_{panel_name}_TOGGLE_ROW_COL"));
        builder.init_variable(variable.clone(), f64::NAN);
        builder.event_to_variable(
            &format!("CB_{panel_name}_TOGGLE"),
            EventToVariableMapping::EventDataEx1ToValue(|values| {
                f64::from_bits((values[0] & 0xFFFFFFFF) as u64 | ((values[1] as u64) << 32))
            }),
            variable,
            |options| options.mask().afterwards_reset_to(f64::NAN),
        )?;
    }

    Ok(())
}
