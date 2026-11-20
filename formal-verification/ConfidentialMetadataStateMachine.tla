-------------------- MODULE ConfidentialMetadataStateMachine --------------------
EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    MaxPayloadBytes,
    MaxProofBytes,
    MaxBatchSize,
    MaxParties

VARIABLES
    nextSeq,
    totalEvents,
    eventLog,
    keyRecords,
    activeKeys

vars == <<nextSeq, totalEvents, eventLog, keyRecords, activeKeys>>

Init ==
    /\ nextSeq = 1
    /\ totalEvents = 0
    /\ eventLog = <<>>
    /\ keyRecords = [u \in {} |-> {}]
    /\ activeKeys = {}

InvSequenceMonotonicity ==
    /\ nextSeq = totalEvents + 1
    /\ \A i \in 1..Len(eventLog) : eventLog[i].seq = i

InvTotalEventsCount ==
    totalEvents = Len(eventLog)

InvKeyLifecycle ==
    activeKeys \subseteq DOMAIN keyRecords

InvPayloadBounds ==
    \A i \in 1..Len(eventLog) :
        /\ eventLog[i].payloadSize <= MaxPayloadBytes
        /\ eventLog[i].proofSize <= MaxProofBytes
        /\ eventLog[i].proofSize >= 16

StateInvariant ==
    /\ InvSequenceMonotonicity
    /\ InvTotalEventsCount
    /\ InvKeyLifecycle
    /\ InvPayloadBounds
=============================================================================
