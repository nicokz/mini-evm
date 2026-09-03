use ruint::aliases::U256;

pub struct Stack<const CAP: usize = 1024> {
    data: [U256; CAP],
    sp: usize,
}

impl<const CAP: usize> Stack<CAP> {
    pub const fn new() -> Self {
        Self {
            data: [U256::ZERO; CAP],
            sp: 0,
        }
    }

    #[inline(always)]
    pub fn push(&mut self, val: U256) -> Result<(), &'static str> {
        if self.sp >= CAP {
            return Err("Stack Overflow");
        }
        self.data[self.sp] = val;
        self.sp += 1;
        Ok(())
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Result<U256, &'static str> {
        if self.sp == 0 {
            return Err("Stack Underflow");
        }
        self.sp -= 1;
        Ok(self.data[self.sp])
    }

    #[inline(always)]
    pub fn peek(&self, depth: usize) -> Result<U256, &'static str> {
        if depth == 0 || self.sp < depth {
            return Err("Stack Underflow on Peek");
        }
        Ok(self.data[self.sp - depth])
    }

    #[inline(always)]
    pub fn swap(&mut self, depth: usize) -> Result<(), &'static str> {
        if depth == 0 || self.sp <= depth {
            return Err("Stack Underflow on Swap");
        }
        let top = self.sp - 1;
        let target = self.sp - 1 - depth;
        self.data.swap(top, target);
        Ok(())
    }

    pub fn values(&self) -> Vec<U256> {
        self.data[..self.sp].to_vec()
    }
}

impl<const CAP: usize> Default for Stack<CAP> {
    fn default() -> Self {
        Self::new()
    }
}
