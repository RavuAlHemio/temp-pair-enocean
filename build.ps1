[CmdletBinding()]
Param(
    [switch]$NoProgram
)

& cargo build --release
if ($LASTEXITCODE -ne 0) {
    Throw "Rust build failed"
}

& rust-objcopy.exe --output-target binary .\target\thumbv7em-none-eabihf\release\temp-pair-enocean temp-pair-enocean.bin
If ($LASTEXITCODE -ne 0) {
    Throw "ELF-to-bin conversion failed"
}

If (-not $NoProgram) {
    & 'C:\Program Files\OpenOCD\bin\openocd.exe' `
        -c "source mikroe-click4-stm32f74.cfg" `
        -c "program temp-pair-enocean.bin 0x08000000 verify reset exit"
}
