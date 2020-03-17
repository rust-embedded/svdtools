import os.path

from ..interrupts import main as interrupts

SVD = """
<device>
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
"""

INTERRUPTS = """\
1 INT_A1: Interrupt A1 (in PeriphA)
3 INT_B3: Interrupt B3 (in PeriphB)
Gaps: 0, 2"""


def test_interrupts(tmpdir):
    svd_file = os.path.join(tmpdir, "test.svd")

    with open(svd_file, "w") as f:
        f.write(SVD)

    result = interrupts(svd_file)

    assert result == INTERRUPTS
