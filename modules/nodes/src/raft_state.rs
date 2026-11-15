#![forbid(unsafe_code)]

use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaftRole {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestVote {
    pub term: u64,
    pub candidate_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestVoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendEntries {
    pub term: u64,
    pub leader_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendEntriesResponse {
    pub term: u64,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct RaftNode {
    pub id: u64,
    pub role: RaftRole,
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub votes_received: HashSet<u64>,
    pub total_nodes: usize,
}

impl RaftNode {
    pub fn new(id: u64, total_nodes: usize) -> Self {
        assert!(total_nodes > 0, "Cluster must have at least 1 node");
        Self {
            id,
            role: RaftRole::Follower,
            current_term: 0,
            voted_for: None,
            votes_received: HashSet::new(),
            total_nodes,
        }
    }

    pub fn step_down(&mut self, term: u64) {
        if term > self.current_term {
            self.current_term = term;
            self.voted_for = None;
        }
        self.role = RaftRole::Follower;
        self.votes_received.clear();
    }

    pub fn start_election(&mut self) -> RequestVote {
        self.step_down(self.current_term + 1); // Advances term, becomes follower temporarily
        self.role = RaftRole::Candidate; // Becomes candidate
        self.voted_for = Some(self.id);
        self.votes_received.insert(self.id);

        if self.has_quorum() {
            self.role = RaftRole::Leader;
        }

        RequestVote {
            term: self.current_term,
            candidate_id: self.id,
        }
    }

    pub fn has_quorum(&self) -> bool {
        self.votes_received.len() > self.total_nodes / 2
    }

    pub fn handle_request_vote(&mut self, req: RequestVote) -> RequestVoteResponse {
        if req.term > self.current_term {
            self.step_down(req.term);
        }

        let mut vote_granted = false;
        if req.term == self.current_term {
            if self.voted_for.is_none() || self.voted_for == Some(req.candidate_id) {
                self.voted_for = Some(req.candidate_id);
                vote_granted = true;
            }
        }

        RequestVoteResponse {
            term: self.current_term,
            vote_granted,
        }
    }

    pub fn handle_request_vote_response(&mut self, voter_id: u64, resp: RequestVoteResponse) {
        if resp.term > self.current_term {
            self.step_down(resp.term);
            return;
        }

        if self.role == RaftRole::Candidate && resp.term == self.current_term && resp.vote_granted {
            self.votes_received.insert(voter_id);
            if self.has_quorum() {
                self.role = RaftRole::Leader;
            }
        }
    }

    pub fn handle_append_entries(&mut self, req: AppendEntries) -> AppendEntriesResponse {
        if req.term > self.current_term {
            self.step_down(req.term);
        }

        let success = if req.term == self.current_term {
            if self.role == RaftRole::Candidate {
                self.role = RaftRole::Follower;
            }
            true
        } else {
            false
        };

        AppendEntriesResponse {
            term: self.current_term,
            success,
        }
    }

    pub fn handle_append_entries_response(&mut self, resp: AppendEntriesResponse) {
        if resp.term > self.current_term {
            self.step_down(resp.term);
        }
    }
}
