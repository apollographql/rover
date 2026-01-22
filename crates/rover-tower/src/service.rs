use tower::Service;

pub fn replace_ready_service<S, T>(src: &mut S) -> S
where
    S: Service<T> + Clone,
{
    let clone = src.clone();
    std::mem::replace(src, clone)
}
