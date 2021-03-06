#![allow(clippy::mem_discriminant_non_enum)]
// spec.rs - MIT License
//  MIT License
//  Copyright (c) 2018 Tyler Laing (ZerothLaw)
//
//  Permission is hereby granted, free of charge, to any person obtaining a copy
//  of this software and associated documentation files (the "Software"), to deal
//  in the Software without restriction, including without limitation the rights
//  to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//  copies of the Software, and to permit persons to whom the Software is
//  furnished to do so, subject to the following conditions:
//
//  The above copyright notice and this permission notice shall be included in all
//  copies or substantial portions of the Software.
//
//  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//  IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//  FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//  AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//  LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//  OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//  SOFTWARE.

//! # ParserSpec
//! The parser spec encapsulates the mapping of tokens with null/left/right binding
//! precedences, as well as the actual syntax rule execution (such as a recursive
//! call to the parser.)
//!
//! ## Closure types:
//! NullDenotation<T> = fn(&mut dyn Parser<T>, T, u32) -> Result<SimpleNode<T>, ParseError<T>>;
//! LeftDenotation<T> = fn(&mut dyn Parser<T>, T, u32, SimpleNode<T>) -> Result<SimpleNode<T>, ParseError<T>>;
//!
//! where T is your token type.
//!
//! Tokens must implement the required traits:
//!     Clone + Debug + Display + PartialEq
//!
//! Send + Sync + 'static are inherent and auto-implemented by the compiler on valid Token types.
//!
//! ## Notes
//! ParserSpec utilizes a "WriteOnce" pattern with the HashMaps where only the first
//! token -> syntax rule is recorded. This means later attempts to reassign the
//! token -> syntax rule mapping are cause an error.
//!

use std::collections::HashMap;
use std::marker::{Send, Sync};
use std::mem::{discriminant, Discriminant};

use node::SimpleNode;
use precedence::PrecedenceLevel;
use token::Token;
use types::*;

/// This currently only indicates if your specification attempts to assign
/// more than one syntax rule to the same token, thus ending early before
/// trying to debug a bad parse.
#[derive(Clone, Debug, Fail)]
pub enum SpecificationError<T: Token + Send + Sync + 'static> {
    #[fail(display = "{} token -> rule mapping was already defined", tk)]
    TokenToRuleAlreadyDefined { tk: T },
}

#[derive(Clone)]
pub struct ParserSpec<T: Token + Send + Sync + 'static, Node = SimpleNode<T>> {
    null_map: HashMap<Discriminant<T>, NullInfo<T, Node>>,
    left_map: HashMap<Discriminant<T>, LeftInfo<T, Node>>,
}

impl<T: Token + Send + Sync + 'static, Node> Default for ParserSpec<T, Node> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Token + Send + Sync + 'static, Node> ParserSpec<T, Node> {
    pub fn new() -> Self {
        ParserSpec {
            null_map: HashMap::new(),
            left_map: HashMap::new(),
        }
    }

    pub fn add_null_assoc(
        &mut self,
        token: impl Into<T>,
        bp: PrecedenceLevel,
        func: NullDenotation<T, Node>,
    ) -> Result<(), SpecificationError<T>> {
        let token = token.into();
        let disc = discriminant(&token);
        match self.null_map.entry(disc) {
            std::collections::hash_map::Entry::Occupied(_) => {
                Err(SpecificationError::TokenToRuleAlreadyDefined { tk: token })
            }
            p => {
                p.or_insert((bp, func));
                Ok(())
            }
        }
    }

    #[allow(clippy::map_entry)] // no easy way to implement `else` branch with Entry
    pub fn add_left_assoc(
        &mut self,
        token: impl Into<T>,
        bp: PrecedenceLevel,
        func: LeftDenotation<T, Node>,
    ) -> Result<(), SpecificationError<T>> {
        let token = token.into();
        let disc = discriminant(&token);
        if !self.left_map.contains_key(&disc) {
            self.left_map.insert(disc, (bp, bp, func));
            Ok(())
        } else {
            Err(SpecificationError::TokenToRuleAlreadyDefined { tk: token })
        }
    }

    #[allow(clippy::map_entry)] // see above
    pub fn add_left_right_assoc(
        &mut self,
        token: impl Into<T>,
        lbp: PrecedenceLevel,
        rbp: PrecedenceLevel,
        func: LeftDenotation<T, Node>,
    ) -> Result<(), SpecificationError<T>> {
        let token = token.into();
        let disc = discriminant(&token);
        if !self.left_map.contains_key(&disc) {
            self.left_map.insert(disc, (lbp, rbp, func));
            Ok(())
        } else {
            Err(SpecificationError::TokenToRuleAlreadyDefined { tk: token })
        }
    }

    pub fn add_null_associations(
        &mut self,
        tokens: impl IntoIterator<Item = impl Into<T>>,
        bp: PrecedenceLevel,
        func: NullDenotation<T, Node>,
    ) -> Result<(), SpecificationError<T>> {
        for token in tokens {
            self.add_null_assoc(token, bp, func)?;
        }
        Ok(())
    }

    pub fn add_left_associations(
        &mut self,
        tokens: impl IntoIterator<Item = impl Into<T>>,
        bp: PrecedenceLevel,
        func: LeftDenotation<T, Node>,
    ) -> Result<(), SpecificationError<T>> {
        for token in tokens {
            self.add_left_assoc(token, bp, func)?;
        }
        Ok(())
    }

    pub fn add_left_right_associations(
        &mut self,
        tokens: impl IntoIterator<Item = impl Into<T>>,
        lbp: PrecedenceLevel,
        rbp: PrecedenceLevel,
        func: LeftDenotation<T, Node>,
    ) -> Result<(), SpecificationError<T>> {
        for token in tokens {
            self.add_left_right_assoc(token, lbp, rbp, func)?;
        }
        Ok(())
    }

    ///Consumes a spec and gets the HashMaps used for mapping tokens
    /// to syntax rules. This avoids clones and allocations/deallocations
    /// of potentially large HashMaps when creating a Parser from the maps.
    #[allow(clippy::type_complexity)]
    pub fn maps(
        self,
    ) -> (
        HashMap<Discriminant<T>, NullInfo<T, Node>>,
        HashMap<Discriminant<T>, LeftInfo<T, Node>>,
    ) {
        (self.null_map, self.left_map)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    //Catch Send/Sync changes
    #[test]
    fn test_parserspec_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ParserSpec<String>>();
    }

    #[test]
    fn test_parserspec_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ParserSpec<String>>();
    }
}
