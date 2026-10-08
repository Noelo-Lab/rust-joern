void enable_irq(void)
{
    __asm volatile ("cpsie i" : : : "memory");
}
