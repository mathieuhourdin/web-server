Publication is an important matter on Hupo. People share intimate things in their journals. 
They should be able to :
- Modulate access grants with fine grain
- Trust the platform for technical respect of the annouced behaviors
- Avoid publishing something by lack of attention

Users have three different types of records in the platform :
- Traces : small piece of text attached in the writing flux of a journal by time of writing
- Documents : static document independant of any journal flux. Can be mentioned in a trace.
- Albums : collection of traces

Access control on records are given by Posts.
Each posts have post_grants that allow to calculate access.

Journals have access policies that are used to manage more easily grants on associated posts, but that are not used to calculate individual access to records.


Then the model is the following : 
- Access is calculated from post grants, using status field and post_grants
- Access to a trace propagate to access to any linked document
- Records have some internal status that have a meaning for publication (eg : archived traces). Those status should be synced with post statuses to allow post to hold all sharing informations.
- What is still an a question is access propagation through links between traces and documents.

Record - Post relationship : 
- Record have 0 or 1 Post
- Post has 1 source


Meaning of internal statuses of records are for the record owner : a traces is not shown in his own journal if archived. 
Rule : access for owner are always better than for followers. So an archived record should give an archived post.

Directinality of status cascade are Record to Posts and negative only.
Archiving a record archives the post
Dearchiving a record never re-published the post by itself
Status update on post never updates the record and publishing is rejected if it results in a forbidden state

Internal status field for records : 
Traces (status) :
- Draft : no post associated
- Finalized : published posts allowed
- Archived : only archived posts or no post (Finalized -> Archived trace transition gives Published -> Archived transition for post)

Documents (status) : 
- Active : published posts allowed
- Archived : only archived posts or no posts (transition to archived gives archived post)

Albums (Completion status) :
- Complete / InProgress : published posts allowed
- Archived : only archived posts or no posts (transition to archived gives archived post)

Note : currently we have a visibility field on albums but it should be replaced by a post and it's published / archived status. Working on it currently

Archiving a post doesn't touch the post_grants. Inert cascade on grants.



### Bulk sharing of access to records
sharing old records proceed like this : 
- When I accept a new follower, I need to review history of all_follower jounral_grants to share or not with him
- When I add a new user to journal_sharing_policies, I am asked to review the historic too.

journal_grants are no longer a source of truth for access to traces.
They should be transformed into journal_sharing_policies : 
- Future policy : journal_sharing_policies on a given journal are the default post_grants for traces belonging to this journal.
- History jobs : journal_sharing_policies also hold a user related job state on sharing or not former traces of a given journal. 
Two way a policy is added to a journal : 
- manual addition of a policy for a given user and journal
- automatic addition of a new follower for a all_followers shared journal.

In both cases, the new policy has a history_sharing_job_state to unreviewed first, and no history trace is shared.
Then the user can review the history sharing. He has multiple options : 
- Share every history traces to this user
- Share no history traces to this user
- Share all history traces except sharing_sensitivity = Sensitive
- Share specific history traces 

Then the state passes to reviewed and the user is never asked again to review the history for this user and this journal.
