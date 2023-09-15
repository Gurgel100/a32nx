use crate::simulation::{
    InitContext, Read, SimulationElement, SimulatorReader, SimulatorWriter, VariableIdentifier,
    Write,
};

use super::{ElectricalElement, ElectricalElementIdentifier, ElectricalElementIdentifierProvider};

pub struct CircuitBreakerBoard<const ROWS: usize, const COLUMNS: usize> {
    toggle_var_id: VariableIdentifier,
    breaker_state_ids: [VariableIdentifier; ROWS],

    circuit_breakers: [[CircuitBreaker; COLUMNS]; ROWS],
    circuit_breaker_states: [f64; ROWS],
}
impl<const ROWS: usize, const COLUMNS: usize> CircuitBreakerBoard<ROWS, COLUMNS> {
    pub fn new(context: &mut InitContext, name: &str) -> Self {
        let mut breaker_state_ids = [VariableIdentifier::default(); ROWS];
        for (row, name) in Self::generate_variable_names(name).enumerate() {
            breaker_state_ids[row] = context.get_identifier(name.to_owned())
        }

        Self {
            toggle_var_id: context.get_identifier(format!("CB_{name}_TOGGLE_ROW_COL")),
            breaker_state_ids,
            circuit_breakers: [[0; COLUMNS]; ROWS].map(|c| c.map(|_| CircuitBreaker::new(context))),
            circuit_breaker_states: [0.; ROWS],
        }
    }

    fn generate_variable_names(name: &str) -> impl Iterator<Item = String> + '_ {
        (0..ROWS).map(move |row| {
            let row_ident = char::from_digit(row as u32 + 10, 36)
                .unwrap()
                .to_ascii_uppercase();
            format!("CB_{name}_{row_ident}")
        })
    }

    fn toggle_breaker(&mut self, row: usize, col: usize) {
        self.circuit_breakers.get_mut(row).and_then(|c| {
            c.get_mut(col).and_then(|breaker| {
                breaker.toggle();
                self.circuit_breaker_states[row] = f64::from_bits(
                    self.circuit_breaker_states[row].to_bits()
                        ^ ((breaker.is_tripped() as u64) << col),
                );
                Some(())
            })
        });
    }
}
impl<const ROWS: usize, const COLUMNS: usize> SimulationElement
    for CircuitBreakerBoard<ROWS, COLUMNS>
{
    fn read(&mut self, reader: &mut SimulatorReader) {
        let toggle_row_col: f64 = reader.read(&self.toggle_var_id);
        if !toggle_row_col.is_nan() {
            let toggle_row_col = toggle_row_col.to_bits();
            let row = toggle_row_col & 0xFFFFFFFF;
            let col = toggle_row_col >> 32;
            self.toggle_breaker(row as usize, col as usize);
        }
    }

    fn write(&self, writer: &mut SimulatorWriter) {
        for (identifier, value) in self
            .breaker_state_ids
            .iter()
            .zip(self.circuit_breaker_states)
        {
            writer.write(identifier, value);
        }
    }
}

// TODO: With the current electrical system it's not possible to trip a circuit braker based on the current.
pub struct CircuitBreaker {
    identifier: ElectricalElementIdentifier,
    tripped: bool,
}
impl CircuitBreaker {
    fn new(context: &mut InitContext) -> Self {
        Self {
            identifier: context.next_electrical_identifier(),
            tripped: false,
        }
    }

    fn toggle(&mut self) {
        self.tripped = !self.tripped
    }

    fn is_tripped(&self) -> bool {
        self.tripped
    }
}
impl ElectricalElement for CircuitBreaker {
    fn input_identifier(&self) -> ElectricalElementIdentifier {
        self.identifier
    }

    fn output_identifier(&self) -> ElectricalElementIdentifier {
        self.identifier
    }

    fn is_conductive(&self) -> bool {
        !self.tripped
    }
}

#[cfg(test)]
mod circuit_breaker_tests {
    use super::*;
}
