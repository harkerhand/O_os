.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
    .section .text
    .globl __alltraps
    .globl __restore
    # RISCV 的对齐要求
    .align 2
__alltraps:
    csrrw sp, sscratch, sp # 将用户栈指针保存到 sscratch 寄存器，并将 sp 设置为内核栈的起始位置
    addi sp, sp, -34*8 # 为 TrapContext 分配空间，TrapContext 大小为 34 个 usize
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
    # 设置入参 trap_handler(cx: &mut TrapContext)
    mv a0, sp
    call trap_handler

__restore:
    # 从内核栈恢复 sstatus、sepc 和 sscratch 寄存器
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    ld t2, 2*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2
    # 从内核栈恢复 x1~x31 寄存器，注意跳过 x2(sp) 和 x4(tp)
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # 恢复完寄存器后，释放内核栈空间
    addi sp, sp, 34*8
    # 最后将用户栈指针从 sscratch 寄存器恢复到 sp 寄存器，并执行 sret 指令返回用户态
    csrrw sp, sscratch, sp
    sret
