# Stokes' Theorem

Stokes' theorem says we can calculate the flux of curl **F** across surface *S* by knowing information only about the values of **F** along the boundary of *S*. Conversely, we can calculate the line integral of vector field **F** along the boundary of surface *S* by translating to a double integral of the curl of **F** over *S*.

Let *S* be an oriented smooth surface with unit normal vector **N**. Furthermore, suppose the boundary of *S* is a simple closed curve *C*. The orientation of *S* induces the positive orientation of *C* if, as you walk in the positive direction around *C* with your head pointing in the direction of **N**, the surface is always on your left. With this definition in place, we can state Stokes' theorem.

## Theorem 6.19

#### Stokes' Theorem

Let *S* be a piecewise smooth oriented surface with a boundary that is a simple closed curve *C* with positive orientation ([Figure 6.79](https://openstax.org/books/calculus-volume-3/pages/6-7-stokes-theorem)). If **F** is a vector field with component functions that have continuous partial derivatives on an open region containing *S*, then

$${\int_{C}{\mathbf{\text{F}} \cdot d\mathbf{\text{r}}}} = {\iint_{S}{\text{curl}\ \mathbf{\text{F}} \cdot d\mathbf{\text{S}}}}.$$

Suppose surface *S* is a flat region in the *xy*-plane with upward orientation. Then the unit normal vector is **k** and surface integral ${\iint\limits_{S}{\text{curl}\ \mathbf{\text{F}}}} \cdot d\mathbf{\text{S}}$ is actually the double integral ${\iint\limits_{S}{\text{curl}\ \mathbf{\text{F}}}} \cdot \mathbf{\text{k}}dA.$ In this special case, Stokes' theorem gives ${\int_{C}{\mathbf{\text{F}} \cdot d\mathbf{\text{r}}}} = {\iint_{S}{\text{curl}\ \mathbf{\text{F}} \cdot \mathbf{\text{k}}dA.}}$ However, this is the circulation form of Green's theorem, which shows us that Green's theorem is a special case of Stokes' theorem. Green's theorem can only handle surfaces in a plane, but Stokes' theorem can handle surfaces in a plane or in space.

The complete proof of Stokes' theorem is beyond the scope of this text. We look at an intuitive explanation for the truth of the theorem and then see proof of the theorem in the special case that surface *S* is a portion of a graph of a function, and *S*, the boundary of *S,* and **F** are all fairly tame.

#### Proof

First, we look at an informal proof of the theorem. This proof is not rigorous, but it is meant to give a general feeling for why the theorem is true. Let *S* be a surface and let *D* be a small piece of the surface so that *D* does not share any points with the boundary of *S*. We choose *D* to be small enough so that it can be approximated by an oriented square *E*. Let *D* inherit its orientation from *S*, and give *E* the same orientation. This square has four sides; denote them $E_{l},$ $E_{r},$ $E_{u},$ and $E_{d}$ for the left, right, up, and down sides, respectively. On the square, we can use the flux form of Green's theorem:

$${\int_{E_{l} + E_{d} + E_{r} + E_{u}}{\mathbf{\text{F}} \cdot d\mathbf{\text{r}}}} = {\iint_{E}{\text{curl}\ \mathbf{\text{F}} \cdot \mathbf{\text{N}}dS}} = {\iint_{E}{\text{curl}\ \mathbf{\text{F}} \cdot d\mathbf{\text{S}}}}.$$

To approximate the flux over the entire surface, we add the values of the flux on the small squares approximating small pieces of the surface ([Figure 6.80](https://openstax.org/books/calculus-volume-3/pages/6-7-stokes-theorem)). By Green's theorem, the flux across each approximating square is a line integral over its boundary. Let *F* be an approximating square with an orientation inherited from *S* and with a right side $E_{l}$ (so *F* is to the left of *E*). Let $F_{r}$ denote the right side of $F$; then, $E_{l} = \text{−}F_{r}.$ In other words, the right side of $F$ is the same curve as the left side of *E*, just oriented in the opposite direction. Therefore,

$${\int_{E_{l}}{\mathbf{\text{F}} \cdot d\mathbf{\text{r}}}} = \text{−}{\int_{F_{r}}{\mathbf{\text{F}} \cdot d\mathbf{\text{r}}}}.$$

As we add up all the fluxes over all the squares approximating surface *S*, line integrals $\int_{E_{l}}{\mathbf{\text{F}} \cdot d\mathbf{\text{r}}}$ and $\int_{F_{r}}{\mathbf{\text{F}} \cdot d\mathbf{\text{r}}}$ cancel each other out. The same goes for the line integrals over the other three sides of *E*. These three line integrals cancel out with the line integral of the lower side of the square above *E*, the line integral over the left side of the square to the right of *E*, and the line integral over the upper side of the square below *E* ([Figure 6.81](https://openstax.org/books/calculus-volume-3/pages/6-7-stokes-theorem)). After all this cancelation occurs over all the approximating squares, the only line integrals that survive are the line integrals over sides approximating the boundary of *S*. Therefore, the sum of all the fluxes (which, by Green's theorem, is the sum of all the line integrals around the boundaries of approximating squares) can be approximated by a line integral over the boundary of *S*. In the limit, as the areas of the approximating squares go to zero, this approximation gets arbitrarily close to the flux.
