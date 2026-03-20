A landmark is a fixed point in the user's landscape of work.
It holds a fixed identity accross time and accross multiple traces.

We have two kinds of landmarks : 
- Public landmarks that are shared accross multiple users : published books... 
- Private landmarks that are owned by only one user : reading notes...

A landmark has : 
- title
- description
- landmark_type : PERSON | RESOURCE | PROJECT | ORGANIZATION | DELIVERABLE | TOPIC | TOOL | ROLE | PLACE

We could have : 
- landmark_subtype : BOOK | MOVIE | DELIVERABLE | THESIS | READING_NOTE... We could use this but not implemented
- public_id : an ISBN for a book, ...

But then, do landmark belong to a user ? 
It is the question with the transactions. What really belongs to a given user that reads a book is the long term running transaction of reading this book. The book itself doesn't belong to anyone, except it's author maybe.
However it is overcompilcated for now. We should just keep the landmark as an entity that belongs to a user.
However, I don't think the landmark should evolve that much. It is not a good thing that it is recreated each time it is mentioned. Or actually it could evolve but keeping a stable parent. We could have landmark and landmark_version. This way, landmark_version could hold a summary of the landmark current state. Let's see later, but for now I would like to make landmarks stable.

The evolution we have now is more on the reference side : once a reference is clearly identified, we create a stable landmark that stops moving.

- user_id
- analysis_id


Landmark is referenced by : 

- landscapes : the landscapes it appears in
- references : the references that are express trace references to it.