use quick_xml::de;
use serde::Deserialize;
use std::io::{BufReader, Read};

#[derive(Deserialize, Debug)]
struct Svd {
    peripherals: PeripheralList,
}

#[derive(Deserialize, Debug)]
struct PeripheralList {
    peripheral: Vec<PeripheralXml>,
}

#[derive(Deserialize, Debug)]
struct PeripheralXml {
    name: String,
    // some peripherals have no interrupts
    interrupt: Option<Vec<Interrupt>>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct Interrupt {
    pub name: String,
    pub description: Option<String>,
    pub value: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Peripheral {
    pub name: String,
    pub interrupt: Vec<Interrupt>,
}

/// get all peripherals that contain at least one interrupt
pub fn peripherals_with_interrupts<R: Read>(svd: R) -> impl Iterator<Item = Peripheral> {
    let reader = BufReader::new(svd);
    let svd: Svd = de::from_reader(reader).expect("svd not formatted correctly");

    let peripheral_list = svd.peripherals.peripheral;

    peripheral_list.into_iter().filter_map(|p| {
        if let Some(interrupt) = p.interrupt {
            Some(Peripheral {
                name: p.name,
                interrupt,
            })
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    static SVD: &str = r"<device>
    <name>Test Device</name>
    <peripherals>
        <peripheral>
            <name>PeriphA</name>
            <interrupt>
                <name>INT_A1</name>
                <description>Interrupt A1</description>
                <value>1</value>
            </interrupt>
        </peripheral>
        <peripheral>
            <name>PeriphB</name>
            <interrupt>
                <name>INT_B3</name>
                <description>Interrupt B3</description>
                <value>3</value>
            </interrupt>
        </peripheral>
    </peripherals>
</device>
";

    #[test]
    fn peripherals_interrupts_are_parsed_correctly() {
        let svd = SVD.as_bytes();
        let actual_peripherals: Vec<Peripheral> = peripherals_with_interrupts(svd).collect();
        let expected_peripherals = vec![
            Peripheral {
                name: "PeriphA".to_string(),
                interrupt: vec![Interrupt {
                    name: "INT_A1".to_string(),
                    description: Some("Interrupt A1".to_string()),
                    value: 1,
                }],
            },
            Peripheral {
                name: "PeriphB".to_string(),
                interrupt: vec![Interrupt {
                    name: "INT_B3".to_string(),
                    description: Some("Interrupt B3".to_string()),
                    value: 3,
                }],
            },
        ];

        assert_eq!(actual_peripherals, expected_peripherals);
    }
}
