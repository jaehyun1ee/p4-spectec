use std::fmt;

use super::SimError;

pub type Port = i32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rx {
    pub port: Port,
    pub packet: String,
}

impl Rx {
    pub fn new(port: Port, packet: impl Into<String>) -> Self {
        Self {
            port,
            packet: packet.into().to_ascii_uppercase(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tx {
    pub port: Port,
    pub packet: String,
}

impl Tx {
    pub fn new(port: Port, packet: impl Into<String>) -> Self {
        Self {
            port,
            packet: packet.into().to_ascii_uppercase(),
        }
    }
}

impl fmt::Display for Tx {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "({})", self.port)?;
        if !self.packet.is_empty() {
            write!(formatter, " {}", self.packet)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Expectation {
    pub tx: Tx,
    pub exact: bool,
}

impl Expectation {
    pub fn new(port: Port, packet: impl Into<String>, exact: bool) -> Self {
        Self {
            tx: Tx::new(port, packet),
            exact,
        }
    }
}

pub fn compare_packet(exact: bool, packet_output: &str, packet_expected: &str) -> bool {
    if packet_output.len() < packet_expected.len() {
        return false;
    }
    if exact && packet_output.len() != packet_expected.len() {
        return false;
    }
    packet_output
        .bytes()
        .zip(packet_expected.bytes())
        .all(|(output, expected)| expected == b'*' || output == expected)
}

fn mismatch(expected: &Tx, output: &Tx) -> SimError {
    SimError::message(format!("expected {expected} but got {output}"))
}

#[derive(Debug, Default)]
pub struct PacketIo {
    outputs: Vec<Tx>,
    expectations: Vec<Expectation>,
}

impl PacketIo {
    pub fn outputs(&self) -> &[Tx] {
        &self.outputs
    }

    pub fn expectations(&self) -> &[Expectation] {
        &self.expectations
    }

    pub fn push_output(&mut self, output: Tx) -> Result<Option<Tx>, SimError> {
        let Some(index) = self
            .expectations
            .iter()
            .position(|expectation| expectation.tx.port == output.port)
        else {
            self.outputs.push(output);
            return Ok(None);
        };
        let expectation = &self.expectations[index];
        if !compare_packet(expectation.exact, &output.packet, &expectation.tx.packet) {
            return Err(mismatch(&expectation.tx, &output));
        }
        self.expectations.remove(index);
        Ok(Some(output))
    }

    pub fn push_outputs(
        &mut self,
        outputs: impl IntoIterator<Item = Tx>,
    ) -> Result<Vec<Tx>, SimError> {
        let mut matched = Vec::new();
        for output in outputs {
            if let Some(output) = self.push_output(output)? {
                matched.push(output);
            }
        }
        Ok(matched)
    }

    pub fn push_expectation(&mut self, expectation: Expectation) -> Result<Option<Tx>, SimError> {
        let Some(index) = self
            .outputs
            .iter()
            .position(|output| output.port == expectation.tx.port)
        else {
            self.expectations.push(expectation);
            return Ok(None);
        };
        let output = &self.outputs[index];
        if !compare_packet(expectation.exact, &output.packet, &expectation.tx.packet) {
            return Err(mismatch(&expectation.tx, output));
        }
        Ok(Some(self.outputs.remove(index)))
    }

    pub fn finish(&self) -> Result<(), SimError> {
        if self.outputs.is_empty() && self.expectations.is_empty() {
            return Ok(());
        }
        let mut message = String::new();
        if !self.outputs.is_empty() {
            message.push_str("[FAIL] Remaining packets to be matched:\n");
            message.push_str(
                &self
                    .outputs
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        if !self.expectations.is_empty() {
            message.push_str("[FAIL] Expected packets to be output:\n");
            message.push_str(
                &self
                    .expectations
                    .iter()
                    .map(|expectation| expectation.tx.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        Err(SimError::message(message))
    }
}
