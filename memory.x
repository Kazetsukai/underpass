MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100 - 128K
    /* 100 bytes for bootloader, leave 128K at the end for persisting data */
}
