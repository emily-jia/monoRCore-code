use core::marker::PhantomData;
use core::option::Option;

use crate::{ACCESS_FLAG, DIRTY_FLAG};
use crate::plugins::{Manage, handle_local_pagefault};
use crate::frame_allocator::FrameTracker;
use kernel_vm::{AddressSpace, VmMeta, PageManager, VPN, Pte};
use alloc::collections::VecDeque;

pub struct ClockImproveManager<Meta: VmMeta, M: PageManager<Meta>> {
    queue: VecDeque<(VPN<Meta>, FrameTracker)>,
    ptr: usize,
    dummy: PhantomData<M>
}

impl<Meta: VmMeta, M: PageManager<Meta>> ClockImproveManager<Meta, M> {
    fn get_pte(memory_set: &mut AddressSpace<Meta, M>, vpn: &VPN<Meta>) -> Option<Pte<Meta>> {
        memory_set.translate_to_pte(vpn.base())
    }

    fn get_accessed_dirty(memory_set: &mut AddressSpace<Meta, M>, vpn: &VPN<Meta>) -> (bool, bool) {
        let pte = Self::get_pte(memory_set, vpn).unwrap();
        let flags = pte.flags().val();
        ((flags & ACCESS_FLAG) != 0, (flags & DIRTY_FLAG) != 0)
    }

    fn clear_accessed_and_dirty(memory_set: &mut AddressSpace<Meta, M>, vpn: &VPN<Meta>) {
        memory_set.clear_accessed_and_dirty(*vpn);
    }
}

impl<Meta: VmMeta, M: PageManager<Meta>> Clone for ClockImproveManager<Meta, M> {
    fn clone(&self) -> Self {
        Self { queue: self.queue.clone(), ptr: self.ptr, dummy: PhantomData }
    }
}

impl<Meta: VmMeta, M: PageManager<Meta>> Manage<Meta, M> for ClockImproveManager<Meta, M> {
    fn new() -> Self {
        Self { queue: VecDeque::new(), ptr: 0, dummy: PhantomData }
    }

    fn handle_pagefault(&mut self, memory_set: &mut AddressSpace<Meta, M>, vpn: VPN<Meta>, task_id: usize) {
        handle_local_pagefault(memory_set, vpn, task_id, self)
    }

    fn insert_frame(&mut self, vpn: VPN<Meta>, frame: FrameTracker) {
        self.queue.push_back((vpn, frame))
    }

    fn get_next_frame(&mut self, memory_set: &mut AddressSpace<Meta, M>) -> Option<VPN<Meta>> {
        assert!(self.queue.len() != 0);
        loop {
            if self.ptr >= self.queue.len() {
                self.ptr = 0;
            }
            let (vpn, frame) = &self.queue[self.ptr];

            let (accessed, dirty) = Self::get_accessed_dirty(memory_set, vpn);
            if accessed || dirty {
                Self::clear_accessed_and_dirty(memory_set, vpn);
                self.ptr += 1;
            } else {
                let (vpn, _) = self.queue.remove(self.ptr).unwrap();
                return Some(vpn);
            }
        }
    }
}