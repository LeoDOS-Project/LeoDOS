//! NOS3 hardware library (hwlib) bindings.
//!
//! Re-exports the raw FFI types and functions generated from the
//! NOS3 `hwlib` headers. These provide bus-level access to UART,
//! I2C, SPI, CAN, GPIO, sockets, memory, and torquer interfaces
//! as used by NOS3 component flight software.
//!
//! All symbols are `unsafe` C functions and opaque structs — safe
//! wrappers should be built on top as needed.

pub use crate::ffi::{
    // UART
    uart_access_flag,
    uart_info_t,
    uart_init_port,
    uart_bytes_available,
    uart_flush,
    uart_read_port,
    uart_write_port,
    uart_close_port,
    // I2C
    i2c_bus_info_t,
    i2c_master_init,
    i2c_master_transaction,
    i2c_read_transaction,
    i2c_write_transaction,
    i2c_master_close,
    // SPI
    spi_info_t,
    spi_mutex_t,
    spi_init_dev,
    spi_set_mode,
    spi_get_mode,
    spi_write,
    spi_read,
    spi_transaction,
    spi_select_chip,
    spi_unselect_chip,
    spi_close_device,
    // CAN
    canid_t,
    can_info_t,
    can_init_dev,
    can_set_modes,
    can_write,
    can_read,
    can_close_device,
    can_master_transaction,
    // GPIO
    gpio_info_t,
    gpio_init,
    gpio_read,
    gpio_write,
    gpio_close,
    // Socket
    addr_fam_e,
    type_e,
    category_e,
    socket_info_t,
    socket_create,
    socket_listen,
    socket_accept,
    socket_connect,
    socket_send,
    socket_recv,
    socket_close,
    // Memory
    devmem_write,
    devmem_read,
    // Torquer
    trq_info_t,
    trq_init,
    trq_command,
    trq_close,
    trq_set_time_high,
    trq_set_period,
    trq_set_direction,
};
