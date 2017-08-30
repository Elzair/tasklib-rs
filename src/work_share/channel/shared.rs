use super::task as tc;
use super::super::util;

pub fn make_shared_channels(nt: usize)
                        -> (Vec<Vec<tc::Sender>>,
                            Vec<Vec<tc::Receiver>>)
{
    let ntsq = nt * nt;
    
    // Model the channels as several NxN matrices.
    let mut tasks_tx = Vec::<Option<tc::Sender>>::with_capacity(ntsq);
    let mut tasks_rx = Vec::<Option<tc::Receiver>>::with_capacity(ntsq);
    
    for n in 0..ntsq {
        match util::is_diagonal(n, nt) {
            true => {
                tasks_tx.push(None);
                tasks_rx.push(None);
            },
            false => {
                let (tx, rx) = tc::channel();
                tasks_tx.push(Some(tx));
                tasks_rx.push(Some(rx));
            },
        }
    }

    // Give one part of each channel to its corresponding thread.
    // This is accomplished by transposing the `tasks_rx` 'matrices'.
    for n in 0..ntsq {
        if util::is_lower_left(n, nt) {
            tasks_rx.swap(n, util::get_transpose_index(n, nt));
        }
    }

    (
        util::split_vec(util::filter_vec(tasks_tx), nt, nt-1),
        util::split_vec(util::filter_vec(tasks_rx), nt, nt-1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    static NT: usize = 3;

    #[test]
    fn test_make_shared_channels() {
        let (tasks_tx, tasks_rx) = make_shared_channels(NT);
        assert_eq!(tasks_tx.len(), NT);
        assert_eq!(tasks_rx.len(), NT);
        
        for v in tasks_tx.iter() {
            assert_eq!(v.len(), NT-1);
        }

        for v in tasks_rx.iter() {
            assert_eq!(v.len(), NT-1);
        }
    }

}
