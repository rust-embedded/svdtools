use crate::interrupts::svd_reader::{Interrupt, Peripheral};

pub struct InterruptWithPeriph {
    pub peripheral: String,
    pub interrupt: Interrupt,
}

pub struct InterruptList {
    ordered_interrupts: Vec<InterruptWithPeriph>,
}

impl InterruptList {
    pub fn new(peripherals: impl Iterator<Item = Peripheral>) -> InterruptList {
        let ordered_interrupts = InterruptList::get_ordered_interrupts(peripherals);
        InterruptList { ordered_interrupts }
    }

    /// Get interrupts ordered by interrupt value
    pub fn ordered(&self) -> &[InterruptWithPeriph] {
        &self.ordered_interrupts
    }

    /// Get missing interrupt values
    pub fn gaps(&self) -> Vec<u32> {
        let mut gaps = Vec::new();
        let mut last_interrupt: i64 = -1;
        for i in &self.ordered_interrupts {
            let curr_interrupt = i.interrupt.value;
            let required_interrupt = (last_interrupt + 1) as u32;
            for gap in required_interrupt..curr_interrupt {
                gaps.push(gap);
            }
            last_interrupt = curr_interrupt as i64;
        }
        gaps
    }

    fn get_ordered_interrupts(
        peripherals: impl Iterator<Item = Peripheral>,
    ) -> Vec<InterruptWithPeriph> {
        let mut interrupt_list: Vec<_> = peripherals
            .flat_map(|p| {
                let peripheral = p.name;
                p.interrupt.into_iter().map(move |i| InterruptWithPeriph {
                    peripheral: peripheral.clone(),
                    interrupt: i,
                })
            })
            .collect();
        interrupt_list.sort_by_key(|i| i.interrupt.value);
        interrupt_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_gaps_if_zero_and_two_are_missing() {
        let peripherals = vec![
            Peripheral {
                name: "PeriphA".to_string(),
                interrupt: vec![Interrupt {
                    name: "INT_A1".to_string(),
                    description: None,
                    value: 1,
                }],
            },
            Peripheral {
                name: "PeriphB".to_string(),
                interrupt: vec![Interrupt {
                    name: "INT_B3".to_string(),
                    description: None,
                    value: 3,
                }],
            },
        ];
        let interrupt_list = InterruptList::new(peripherals.into_iter());

        let expected_gaps = vec![0, 2];
        let actual_gaps = interrupt_list.gaps();

        assert_eq!(actual_gaps, expected_gaps);
    }
}
