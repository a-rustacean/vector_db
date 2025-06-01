macro_rules! define_new_types {
    ($(struct $name:ident = $type:ty;)+) => {
        $(
            pub struct $name($type);

            impl core::ops::Deref for $name {
                type Target = $type;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl From<$type> for $name {
                fn from(inner: $type) -> Self {
                    Self(inner)
                }
            }

            impl From<$name> for $type {
                fn from(wrapper: $name) -> Self {
                    wrapper.0
                }
            }
        )+
    };
}

define_new_types! {
    struct HNSWLevel = u8;
    struct NeighborIndex = u16;
}
