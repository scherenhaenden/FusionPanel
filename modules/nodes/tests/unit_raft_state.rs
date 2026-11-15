#![forbid(unsafe_code)]

use nodes::raft_state::*;

#[test]
fn test_initial_state() {
    let node = RaftNode::new(1, 3);
    assert_eq!(node.role, RaftRole::Follower);
    assert_eq!(node.current_term, 0);
    assert_eq!(node.voted_for, None);
    assert_eq!(node.votes_received.len(), 0);
}

#[test]
fn test_start_election() {
    let mut node = RaftNode::new(1, 3);
    let req = node.start_election();

    assert_eq!(node.role, RaftRole::Candidate);
    assert_eq!(node.current_term, 1);
    assert_eq!(node.voted_for, Some(1));
    assert_eq!(node.votes_received.len(), 1);
    assert!(node.votes_received.contains(&1));

    assert_eq!(req.term, 1);
    assert_eq!(req.candidate_id, 1);
}

#[test]
fn test_election_quorum_success() {
    let mut node = RaftNode::new(1, 3);
    node.start_election(); // term 1, candidate, votes={1}

    let resp = RequestVoteResponse {
        term: 1,
        vote_granted: true,
    };
    node.handle_request_vote_response(2, resp);

    assert_eq!(node.role, RaftRole::Leader);
    assert_eq!(node.votes_received.len(), 2);
}

#[test]
fn test_election_single_node_cluster() {
    let mut node = RaftNode::new(1, 1);
    node.start_election();
    assert_eq!(node.role, RaftRole::Leader);
}

#[test]
fn test_handle_request_vote_grant() {
    let mut node = RaftNode::new(1, 3);

    let req = RequestVote { term: 1, candidate_id: 2 };
    let resp = node.handle_request_vote(req);

    assert_eq!(resp.vote_granted, true);
    assert_eq!(resp.term, 1);
    assert_eq!(node.current_term, 1);
    assert_eq!(node.voted_for, Some(2));
}

#[test]
fn test_handle_request_vote_reject_stale_term() {
    let mut node = RaftNode::new(1, 3);
    node.current_term = 2;

    let req = RequestVote { term: 1, candidate_id: 2 };
    let resp = node.handle_request_vote(req);

    assert_eq!(resp.vote_granted, false);
    assert_eq!(resp.term, 2);
}

#[test]
fn test_handle_request_vote_reject_already_voted() {
    let mut node = RaftNode::new(1, 3);
    node.current_term = 1;
    node.voted_for = Some(3); // Already voted for 3 in term 1

    let req = RequestVote { term: 1, candidate_id: 2 };
    let resp = node.handle_request_vote(req);

    assert_eq!(resp.vote_granted, false);
    assert_eq!(resp.term, 1);
}

#[test]
fn test_handle_request_vote_grant_newer_term() {
    let mut node = RaftNode::new(1, 3);
    node.current_term = 1;
    node.voted_for = Some(3);

    let req = RequestVote { term: 2, candidate_id: 2 };
    let resp = node.handle_request_vote(req);

    assert_eq!(resp.vote_granted, true);
    assert_eq!(resp.term, 2);
    assert_eq!(node.current_term, 2);
    assert_eq!(node.voted_for, Some(2));
}

#[test]
fn test_handle_request_vote_response_stale_term() {
    let mut node = RaftNode::new(1, 3);
    node.start_election(); // term 1

    let resp = RequestVoteResponse { term: 0, vote_granted: true };
    node.handle_request_vote_response(2, resp);

    assert_eq!(node.role, RaftRole::Candidate);
    assert_eq!(node.votes_received.len(), 1); // Ignore vote for stale term
}

#[test]
fn test_handle_request_vote_response_newer_term() {
    let mut node = RaftNode::new(1, 3);
    node.start_election(); // term 1, Candidate

    let resp = RequestVoteResponse { term: 2, vote_granted: false };
    node.handle_request_vote_response(2, resp);

    assert_eq!(node.role, RaftRole::Follower);
    assert_eq!(node.current_term, 2);
    assert_eq!(node.voted_for, None);
}

#[test]
fn test_handle_append_entries_success() {
    let mut node = RaftNode::new(1, 3);
    node.current_term = 1;

    let req = AppendEntries { term: 1, leader_id: 2 };
    let resp = node.handle_append_entries(req);

    assert_eq!(resp.success, true);
    assert_eq!(resp.term, 1);
    assert_eq!(node.current_term, 1);
}

#[test]
fn test_handle_append_entries_newer_term() {
    let mut node = RaftNode::new(1, 3);
    node.current_term = 1;
    node.start_election(); // Candidate, term 2

    // Leader in term 3 appears
    let req = AppendEntries { term: 3, leader_id: 2 };
    let resp = node.handle_append_entries(req);

    assert_eq!(resp.success, true);
    assert_eq!(resp.term, 3);
    assert_eq!(node.role, RaftRole::Follower);
    assert_eq!(node.current_term, 3);
}

#[test]
fn test_handle_append_entries_same_term_candidate_step_down() {
    let mut node = RaftNode::new(1, 3);
    node.current_term = 1;
    node.start_election(); // Candidate, term 2

    let req = AppendEntries { term: 2, leader_id: 2 };
    let resp = node.handle_append_entries(req);

    assert_eq!(resp.success, true);
    assert_eq!(resp.term, 2);
    assert_eq!(node.role, RaftRole::Follower);
}

#[test]
fn test_handle_append_entries_reject_stale_term() {
    let mut node = RaftNode::new(1, 3);
    node.current_term = 2;

    let req = AppendEntries { term: 1, leader_id: 2 };
    let resp = node.handle_append_entries(req);

    assert_eq!(resp.success, false);
    assert_eq!(resp.term, 2);
}

#[test]
fn test_handle_append_entries_response_newer_term() {
    let mut node = RaftNode::new(1, 3);
    node.start_election(); // term 1
    node.handle_request_vote_response(2, RequestVoteResponse { term: 1, vote_granted: true }); // Leader
    assert_eq!(node.role, RaftRole::Leader);

    let resp = AppendEntriesResponse { term: 2, success: false };
    node.handle_append_entries_response(resp);

    assert_eq!(node.role, RaftRole::Follower);
    assert_eq!(node.current_term, 2);
}
