// Copyright 2018-2019 Matthieu Felix
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// Checks that a vector has at least `min`, at most `max` entries.
pub fn check_len<T>(v: &[T], min: Option<usize>, max: Option<usize>) -> Result<(), String> {
    if let Some(m) = min {
        if v.len() < m {
            return Err(format!("Too few values, expecting at least {}.", m));
        }
    };
    if let Some(m) = max {
        if v.len() > m {
            return Err(format!("Too many values, expecting at most {}.", m));
        }
    };
    Ok(())
}

/// Extracts a single element from a length 1 vector, or fails.
pub fn extract_single<T>(mut v: Vec<T>) -> Result<T, String> {
    if v.len() > 1 {
        return Err(format!("Too many values, expecting 1 and got {}.", v.len()));
    }
    v.pop()
        .ok_or_else(|| "Too few values, expecting one and got 0.".into())
}
