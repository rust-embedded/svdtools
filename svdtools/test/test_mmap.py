import os.path

from ..mmap import main as mmap

SVD = """
<device>
    <peripherals>
        <peripheral>
            <name>PeriphA</name>
            <description>Peripheral A</description>
            <baseAddress>0x10000000</baseAddress>
            <interrupt>
                <name>INT_A1</name>
                <description>Interrupt A1</description>
                <value>1</value>
            </interrupt>
            <registers>
                <register>
                    <name>REG1</name>
                    <addressOffset>0x10</addressOffset>
                    <description>Register A1</description>
                    <fields>
                        <field>
                            <name>F1</name>
                            <description>Field 1</description>
                            <bitOffset>5</bitOffset>
                            <bitWidth>2</bitWidth>
                        </field>
                        <field>
                            <name>F2</name>
                            <description>Field 2</description>
                            <bitOffset>10</bitOffset>
                            <bitWidth>1</bitWidth>
                        </field>
                    </fields>
                </register>
                <register>
                    <name>REG2</name>
                    <addressOffset>0x14</addressOffset>
                    <description>Register A2</description>
                    <fields>
                    </fields>
                </register>
            </registers>
        </peripheral>
        <peripheral>
            <name>PeriphB</name>
            <description>Peripheral B</description>
            <baseAddress>0x10010000</baseAddress>
            <interrupt>
                <name>INT_B2</name>
                <description>Interrupt B2</description>
                <value>2</value>
            </interrupt>
            <registers>
                <register>
                    <name>REG1</name>
                    <addressOffset>0x10</addressOffset>
                    <description>Register B1</description>
                    <fields>
                    </fields>
                </register>
            </registers>
        </peripheral>
    </peripherals>
</device>
"""

MMAP = """\
0x10000000 A PERIPHERAL PeriphA
0x10000010 B  REGISTER REG1: Register A1
0x10000010 C   FIELD 05w02 F1: Field 1
0x10000010 C   FIELD 10w01 F2: Field 2
0x10000014 B  REGISTER REG2: Register A2
0x10010000 A PERIPHERAL PeriphB
0x10010010 B  REGISTER REG1: Register B1
INTERRUPT 001: INT_A1 (PeriphA): Interrupt A1
INTERRUPT 002: INT_B2 (PeriphB): Interrupt B2\
"""


def test_mmap(tmpdir):
    svd_file = os.path.join(tmpdir, "test.svd")

    with open(svd_file, "w") as f:
        f.write(SVD)

    result = mmap(svd_file)

    assert result == MMAP
