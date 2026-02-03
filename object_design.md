
### The target model

The plateform has three important blocs plus one system bloc.

User (should find another name. Events, Trace system...)
The part that is user defined. It is immutable and it is the source of truth of all the system.
- User
- Journal : A global context for traces.
- Trace : A text content the user writes in a journal, about anything, at a given time.

Analytic
The analytic view, or landscape, that is created from the user's traces.
- TraceMirror : It is the global analysis for the trace it mirrors.
- Element : It is a local analysis for a part of the trace.
- Landmark : It is a stable artefact during time. It has a collection of elements and trace mirrors reltated to it.
- Landscape : At a given time, it is the view of the work state. The landscape is a collection of different landmarks. A landscape is a function of a parent landscape and of a trace that is an event in the world state.
- Analysis : Job of running an analysis. A successfull analysis creates a landscape. (holds the analysis job. Should also have some logs of the pipeline execution for improvement loops)
- Lens : It is a coherent track of analysis.

Social
What the user shows of it's work, and how other people interact with it.
- Resource (any kind of posts)
- Interaction (view, like, saved...)

System
- Sessions
- LLM calls
