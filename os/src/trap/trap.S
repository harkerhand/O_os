.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    # RISCV 的对齐要求
    .align 2
__alltraps:
    # 将用户栈指针保存到 sscratch 寄存器，并将 sp 设置为内核栈的起始位置
    csrrw sp, sscratch, sp 
    # 保存 x1~x31 寄存器到内核栈，注意跳过 x2(sp) 和 x4(tp)，因为它们分别用于保存用户栈指针和线程指针
    sd x1, 1*8(sp)
    sd x3, 3*8(sp)
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    # 保存 sstatus 和 sepc 寄存器到内核栈
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    # 保存 sscratch 寄存器到内核栈，sscratch 中保存了用户栈指针
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # 从内核栈加载 kernel_satp
    ld t0, 34*8(sp) 
    # 从内核栈加载 trap_handler
    ld t1, 36*8(sp)
    # 从内核栈加载 kernel_sp
    ld sp, 35*8(sp)
    # 切换到内核页表
    csrw satp, t0
    sfence.vma
    # 跳转到 trap_handler
    jr t1

__restore:
    csrw satp, a1
    sfence.vma
    csrw sscratch, a0
    mv sp, a0
    # 从内核栈恢复 sstatus、sepc 
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    # 从内核栈恢复 x1~x31 寄存器，注意跳过 x2(sp) 和 x4(tp)
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    ld sp, 2*8(sp)
    sret
