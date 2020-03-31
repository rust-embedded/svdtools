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
        let mut interrupt_list_iter = self.ordered_interrupts.iter().peekable();
        while let Some(i) = interrupt_list_iter.next() {
            let curr_num = i.interrupt.value;
            if let Some(i) = interrupt_list_iter.peek() {
                let next_num = i.interrupt.value;
                for k in (curr_num + 1)..next_num {
                    gaps.push(k);
                }
            }
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
